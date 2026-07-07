use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{
    F95LoginRequest, F95LoginResult, F95SearchResult, Game, GameAttachments, GameDetail,
    GamePlatformArchive, GameResponse, MatchRequest, MigrationStatus, ReorganizeResult,
    ScanResult, ScreenshotItem, Settings, UpdateSettingsRequest, VersionCheckResult,
};
use crate::scanner::{guess_search_queries, scan_archive_folder};
use crate::sources::f95zone::{self, auth, cache_thread_media, text, F95Client};
use std::collections::HashSet;
use std::sync::Arc;

pub struct AppState {
    pub db: Database,
}

impl AppState {
    pub fn new(data_dir: &std::path::Path) -> AppResult<Self> {
        let state = Self {
            db: Database::new(data_dir)?,
        };

        if let Ok(settings) = state.db.get_settings() {
            if !settings.archive_path.trim().is_empty() {
                match crate::migration::reorganize_all(&state.db, &settings.archive_path, false) {
                    Ok(result) if result.moved > 0 || result.failed > 0 => {
                        tracing::info!(
                            moved = result.moved,
                            skipped_unknown = result.skipped_unknown,
                            failed = result.failed,
                            "startup archive reorganization"
                        );
                    }
                    Err(e) => tracing::warn!("startup archive reorganization failed: {e}"),
                    _ => {}
                }
            }
        }

        Ok(state)
    }

    pub fn get_settings(&self) -> AppResult<Settings> {
        self.db.get_settings()
    }

    pub async fn update_settings(&self, req: UpdateSettingsRequest) -> AppResult<Settings> {
        let settings = self.db.update_settings(
            req.archive_path,
            req.f95_username,
            req.f95_password,
            req.f95_cookies,
            req.http_auth_username,
            req.http_auth_password,
            req.http_auth_remove,
        )?;

        if let (Some(user), Some(pass)) = (
            settings.f95_username.clone(),
            self.db.get_f95_credentials()?.1,
        ) {
            if !user.trim().is_empty() && !pass.trim().is_empty() {
                if let Ok(cookies) = auth::login(&user, &pass).await {
                    let _ = self.db.update_f95_cookies(&cookies);
                }
            }
        }

        self.db.get_settings()
    }

    pub async fn f95_login(&self, req: F95LoginRequest) -> AppResult<F95LoginResult> {
        let (stored_user, stored_pass) = self.db.get_f95_credentials()?;
        let username = req
            .username
            .or(stored_user)
            .filter(|u| !u.trim().is_empty());
        let password = req
            .password
            .or(stored_pass)
            .filter(|p| !p.trim().is_empty());

        let (username, password) = match (username, password) {
            (Some(u), Some(p)) => (u, p),
            _ => {
                return Ok(F95LoginResult {
                    success: false,
                    message: "F95Zone username and password are required.".into(),
                });
            }
        };

        match auth::login(&username, &password).await {
            Ok(cookies) => {
                self.db.update_f95_cookies(&cookies)?;
                Ok(F95LoginResult {
                    success: true,
                    message: "Successfully logged in to F95Zone.".into(),
                })
            }
            Err(e) => Ok(F95LoginResult {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    pub async fn ensure_f95_client(&self) -> AppResult<F95Client> {
        let settings = self.db.get_settings()?;

        if let Some(cookies) = settings.f95_cookies.filter(|c| !c.trim().is_empty()) {
            let client = F95Client::from_cookies(&cookies)?;
            if client.probe_auth().await.unwrap_or(false) {
                return Ok(client);
            }
        }

        let (username, password) = self.db.get_f95_credentials()?;
        if let (Some(user), Some(pass)) = (username, password) {
            if !user.trim().is_empty() && !pass.trim().is_empty() {
                let cookies = auth::login(&user, &pass).await?;
                self.db.update_f95_cookies(&cookies)?;
                return F95Client::from_cookies(&cookies);
            }
        }

        Err(AppError::BadRequest(
            "F95Zone authentication required. Enter credentials in Settings and click Login.".into(),
        ))
    }

    pub fn list_games(
        &self,
        name_search: Option<String>,
        tag_filter: Option<String>,
        tag_mode: Option<String>,
        play_status_filter: Option<String>,
        min_f95_rating: Option<f64>,
        max_f95_rating: Option<f64>,
        min_user_rating: Option<f64>,
        max_user_rating: Option<f64>,
        sort: Option<String>,
    ) -> AppResult<Vec<Game>> {
        self.db.list_games(
            name_search.as_deref(),
            tag_filter.as_deref(),
            tag_mode.as_deref(),
            play_status_filter.as_deref(),
            min_f95_rating,
            max_f95_rating,
            min_user_rating,
            max_user_rating,
            sort.as_deref(),
        )
    }

    pub fn list_library_tags(&self) -> AppResult<Vec<crate::models::LibraryTag>> {
        self.db.list_matched_tags()
    }

    pub fn get_game(&self, id: i64) -> AppResult<Game> {
        self.db.get_game(id)
    }

    pub fn get_game_detail(&self, id: i64) -> AppResult<GameDetail> {
        let game = self.db.get_game(id)?;
        let media = self.db.list_game_media(id)?;
        let cover_url = self.cover_api_url(&game);
        let cover_full_url = self.cover_full_url_for_game(&game, &media);
        let screenshots = self.screenshots_for_game(&media);
        let is_custom_cover = self.is_custom_cover(&game, &media);
        let attachments = GameAttachments {
            platform_archives: self.db.list_platform_archives(id)?,
            saves: self.db.list_game_saves(id)?,
            patches: self.db.list_game_patches(id)?,
        };
        Ok(GameDetail {
            game,
            cover_url,
            cover_full_url,
            screenshots,
            is_custom_cover,
            attachments,
        })
    }

    fn is_custom_cover(
        &self,
        game: &Game,
        media: &[crate::models::GameMediaRecord],
    ) -> bool {
        let Some(current) = game.cover_image_path.as_ref() else {
            return false;
        };
        let default_cover = media
            .iter()
            .find(|m| m.media_type == "cover")
            .and_then(|m| m.local_path.as_ref());
        match default_cover {
            Some(default) => current != default,
            None => false,
        }
    }

    pub fn reset_game_cover(&self, game_id: i64) -> AppResult<GameResponse> {
        let game = self.db.reset_game_cover(game_id)?;
        self.game_response(game)
    }

    pub fn update_game_user_data(
        &self,
        game_id: i64,
        req: crate::models::UpdateGameUserDataRequest,
    ) -> AppResult<Game> {
        self.db.update_game_user_data(
            game_id,
            req.play_status.as_deref(),
            req.user_rating,
            req.user_notes.as_deref(),
        )
    }

    pub fn get_storage_stats(&self) -> AppResult<crate::models::StorageStats> {
        use crate::storage::{directory_size, file_size, volume_stats};

        let settings = self.db.get_settings()?;
        let data_dir = self.db.data_dir();
        let db_path = data_dir.join("avn-hub.db");

        let archives_bytes = self.db.sum_archive_sizes()?;
        let media_cache_bytes = directory_size(&self.db.media_dir());
        let database_bytes = file_size(&db_path);
        let data_dir_bytes = directory_size(data_dir);

        let archive_path = std::path::Path::new(&settings.archive_path);
        let archive_vol = if settings.archive_path.is_empty() {
            None
        } else {
            volume_stats(archive_path)
        };
        let data_vol = volume_stats(data_dir);

        Ok(crate::models::StorageStats {
            archives_bytes,
            media_cache_bytes,
            database_bytes,
            data_dir_bytes,
            archive_path: settings.archive_path,
            data_dir: settings.data_dir,
            archive_volume_total: archive_vol.map(|v| v.total_bytes),
            archive_volume_available: archive_vol.map(|v| v.available_bytes),
            data_volume_total: data_vol.map(|v| v.total_bytes),
            data_volume_available: data_vol.map(|v| v.available_bytes),
        })
    }

    fn screenshots_for_game(
        &self,
        media: &[crate::models::GameMediaRecord],
    ) -> Vec<ScreenshotItem> {
        media
            .iter()
            .filter(|m| m.media_type == "screenshot")
            .map(|m| ScreenshotItem {
                full_url: text::upgrade_image_url(&m.source_url),
                cached_url: m
                    .local_path
                    .as_ref()
                    .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir())),
            })
            .collect()
    }

    fn cover_full_url_for_game(
        &self,
        game: &Game,
        media: &[crate::models::GameMediaRecord],
    ) -> Option<String> {
        let cover_path = game.cover_image_path.as_ref()?;
        media
            .iter()
            .find(|m| m.local_path.as_deref() == Some(cover_path.as_str()))
            .map(|m| text::upgrade_image_url(&m.source_url))
    }

    pub fn set_game_cover(&self, game_id: i64, screenshot_index: usize) -> AppResult<GameResponse> {
        let game = self.db.set_game_cover(game_id, screenshot_index)?;
        self.game_response(game)
    }

    pub fn game_response(&self, game: Game) -> AppResult<GameResponse> {
        let media = self.db.list_game_media(game.id)?;
        let platform_archives = self.db.list_platform_archives(game.id).unwrap_or_default();
        Ok(GameResponse {
            cover_url: self.cover_api_url(&game),
            cover_full_url: self.cover_full_url_for_game(&game, &media),
            preview_urls: self.preview_urls_for_game_with_media(&game, &media)?,
            platform_archives,
            game,
        })
    }

    pub fn preview_urls_for_game_with_media(
        &self,
        game: &Game,
        media: &[crate::models::GameMediaRecord],
    ) -> AppResult<Vec<String>> {
        let mut urls = Vec::new();
        if let Some(cached) = game
            .cover_image_path
            .as_ref()
            .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir()))
        {
            urls.push(cached);
        } else if let Some(full) = self.cover_full_url_for_game(game, media) {
            urls.push(full);
        }
        for item in media.iter().filter(|m| m.media_type == "screenshot") {
            let display = item
                .local_path
                .as_ref()
                .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir()))
                .unwrap_or_else(|| text::upgrade_image_url(&item.source_url));
            if !urls.contains(&display) {
                urls.push(display);
            }
        }
        Ok(urls)
    }

    pub fn screenshot_urls_for_game(&self, game_id: i64) -> AppResult<Vec<String>> {
        Ok(self
            .db
            .list_game_media(game_id)?
            .into_iter()
            .filter(|m| m.media_type == "screenshot")
            .filter_map(|m| {
                m.local_path
                    .as_ref()
                    .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir()))
            })
            .collect())
    }

    pub fn preview_urls_for_game(&self, game_id: i64) -> AppResult<Vec<String>> {
        let game = self.db.get_game(game_id)?;
        let media = self.db.list_game_media(game_id)?;
        self.preview_urls_for_game_with_media(&game, &media)
    }

    pub fn unmatch_archive(&self, game_id: i64) -> AppResult<Game> {
        self.db.unmatch_archive(game_id)
    }

    pub fn delete_archive(&self, game_id: i64) -> AppResult<()> {
        self.delete_game_entirely(game_id)
    }

    pub fn delete_platform_archive(&self, archive_id: i64) -> AppResult<()> {
        let archive = self.db.get_platform_archive(archive_id)?;
        let settings = self.db.get_settings()?;

        if !archive.path.is_empty()
            && crate::scanner::is_path_under_archive_root(&archive.path, &settings.archive_path)
            && std::path::Path::new(&archive.path).exists()
        {
            std::fs::remove_file(&archive.path)?;
        }

        let game_id = self.db.delete_platform_archive(archive_id)?;
        let game = self.db.get_game(game_id)?;
        if self.db.list_platform_archives(game_id)?.is_empty() && !game.matched {
            self.delete_game_entirely(game_id)?;
        }
        Ok(())
    }

    pub fn delete_game_save(&self, save_id: i64) -> AppResult<()> {
        let save = self.db.delete_game_save(save_id)?;
        if std::path::Path::new(&save.path).exists() {
            std::fs::remove_file(&save.path)?;
        }
        Ok(())
    }

    pub fn delete_game_patch(&self, patch_id: i64) -> AppResult<()> {
        let patch = self.db.delete_game_patch(patch_id)?;
        let settings = self.db.get_settings()?;
        if crate::attachments::is_path_under_root(&patch.path, &settings.archive_path)
            && std::path::Path::new(&patch.path).exists()
        {
            std::fs::remove_file(&patch.path)?;
        }
        Ok(())
    }

    pub fn set_default_platform_archive(&self, archive_id: i64) -> AppResult<Game> {
        self.db.set_default_platform_archive(archive_id)?;
        let archive = self.db.get_platform_archive(archive_id)?;
        self.db.get_game(archive.game_id)
    }

    pub fn get_migration_status(&self) -> AppResult<MigrationStatus> {
        let pending: Vec<_> = self
            .db
            .list_migration_archives()?
            .into_iter()
            .filter(|a| a.is_legacy_path || a.needs_platform)
            .collect();
        let legacy_paths = pending.iter().filter(|a| a.is_legacy_path).count();
        let unknown_platforms = pending.iter().filter(|a| a.needs_platform).count();
        Ok(MigrationStatus {
            total_archives: pending.len(),
            needs_attention: pending.len(),
            legacy_paths,
            unknown_platforms,
            archives: pending,
        })
    }

    pub fn reorganize_legacy_archives(&self) -> AppResult<ReorganizeResult> {
        let settings = self.db.get_settings()?;
        if settings.archive_path.trim().is_empty() {
            return Err(AppError::BadRequest(
                "archive path not configured".into(),
            ));
        }
        crate::migration::reorganize_all(&self.db, &settings.archive_path, false)
    }

    pub fn assign_archive_platform(
        &self,
        archive_id: i64,
        platform: &str,
        reorganize: bool,
    ) -> AppResult<GamePlatformArchive> {
        let settings = self.db.get_settings()?;
        let (archive, _) = self.db.update_archive_platform(archive_id, platform)?;

        if reorganize && archive.platform != "unknown" {
            if settings.archive_path.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "archive path not configured".into(),
                ));
            }
            self.reorganize_game_archives(archive.game_id)?;
            return self.db.get_platform_archive(archive_id);
        }

        Ok(archive)
    }

    fn reorganize_game_archives(&self, game_id: i64) -> AppResult<()> {
        let settings = self.db.get_settings()?;
        let root = settings.archive_path.trim();
        if root.is_empty() {
            return Ok(());
        }
        let mut last_error: Option<crate::error::AppError> = None;
        for archive in self.db.list_platform_archives(game_id)? {
            if archive.platform == "unknown" {
                continue;
            }
            match crate::migration::reorganize_archive_file(&self.db, root, &archive) {
                Ok(_) => {}
                Err(e) => last_error = Some(e),
            }
        }
        if let Some(e) = last_error {
            return Err(e);
        }
        Ok(())
    }

    fn delete_game_entirely(&self, game_id: i64) -> AppResult<()> {
        let settings = self.db.get_settings()?;
        if settings.archive_path.trim().is_empty() {
            return Err(AppError::BadRequest(
                "archive path not configured".into(),
            ));
        }

        let _game = self.db.get_game(game_id)?;
        let archives = self.db.list_platform_archives(game_id)?;
        for archive in &archives {
            if crate::scanner::is_path_under_archive_root(&archive.path, &settings.archive_path)
                && std::path::Path::new(&archive.path).exists()
            {
                let _ = std::fs::remove_file(&archive.path);
            }
        }

        let saves = self.db.list_game_saves(game_id)?;
        for save in &saves {
            if std::path::Path::new(&save.path).exists() {
                let _ = std::fs::remove_file(&save.path);
            }
        }

        let patches = self.db.list_game_patches(game_id)?;
        for patch in &patches {
            if crate::attachments::is_path_under_root(&patch.path, &settings.archive_path)
                && std::path::Path::new(&patch.path).exists()
            {
                let _ = std::fs::remove_file(&patch.path);
            }
        }

        self.db.delete_game_archive(game_id)
    }

    pub fn purge_media_cache(&self) -> AppResult<()> {
        self.db.purge_media_cache()
    }

    pub fn list_archives(&self) -> AppResult<Vec<crate::models::ArchiveEntry>> {
        self.db.list_archives()
    }

    pub async fn scan_archives(&self) -> AppResult<ScanResult> {
        let settings = self.db.get_settings()?;
        if settings.archive_path.trim().is_empty() {
            return Err(AppError::BadRequest(
                "archive path not configured. Set it in Settings first.".into(),
            ));
        }
        scan_archive_folder(&self.db, &settings.archive_path)
    }

    pub async fn search_f95(&self, query: &str, page: u32) -> AppResult<Vec<F95SearchResult>> {
        let client = self.ensure_f95_client().await?;
        let normalized = text::normalize_apostrophes(query.trim());
        let mut results = client.search(&normalized, page).await?;
        if results.is_empty() && normalized.contains('\'') {
            let stripped = text::strip_apostrophes_for_search(&normalized);
            if !stripped.is_empty() && stripped.to_lowercase() != normalized.to_lowercase() {
                results = client.search(&stripped, page).await?;
            }
        }
        for result in &results {
            let _ = self.db.cache_metadata("f95zone", result);
        }
        Ok(results)
    }

    pub async fn resolve_f95_thread(&self, url_or_id: &str) -> AppResult<F95SearchResult> {
        let thread_id = f95zone::parse_f95_thread_id(url_or_id).ok_or_else(|| {
            AppError::BadRequest(
                "Invalid F95Zone thread URL or ID. Paste a link like \
                 https://f95zone.to/threads/game-name.12345/"
                    .into(),
            )
        })?;

        let client = self.ensure_f95_client().await?;
        let thread = client.fetch_thread_metadata(thread_id).await?;
        let list_entry = client.fetch_list_entry(thread_id).await.ok().flatten();
        let result = Self::merge_match_result(None, list_entry, thread);
        let _ = self.db.cache_metadata("f95zone", &result);
        Ok(result)
    }

    pub async fn suggest_matches(
        &self,
        archive_id: Option<i64>,
        archive_path: Option<&str>,
    ) -> AppResult<Vec<F95SearchResult>> {
        let filename = if let Some(id) = archive_id {
            self.db.get_platform_archive(id)?.filename
        } else if let Some(path) = archive_path {
            std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string()
        } else {
            return Err(AppError::BadRequest(
                "archive_id or path required".into(),
            ));
        };

        let queries = guess_search_queries(&filename);
        if queries.is_empty() {
            return Ok(vec![]);
        }

        let client = self.ensure_f95_client().await?;
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        for query in queries {
            match client.search(&query, 1).await {
                Ok(found) => {
                    for item in found {
                        if seen.insert(item.thread_id) {
                            results.push(item);
                        }
                    }
                }
                Err(e) => {
                    if results.is_empty() {
                        return Err(e);
                    }
                    break;
                }
            }
            if results.len() >= 15 {
                break;
            }
        }

        Ok(results)
    }

    fn merge_match_result(
        hint: Option<F95SearchResult>,
        list: Option<F95SearchResult>,
        thread: f95zone::ThreadMetadata,
    ) -> F95SearchResult {
        let thread_tags = thread.result.tags.clone();
        let thread_screenshots = thread.screenshots.clone();
        let thread_all_images = thread.all_images.clone();
        let mut result = thread.result;

        if let Some(ref l) = list {
            if !l.title.is_empty() {
                result.title = l.title.clone();
            }
            if !l.screenshots.is_empty() {
                result.screenshots = l.screenshots.clone();
            } else if result.screenshots.is_empty() && !thread_screenshots.is_empty() {
                result.screenshots = thread_screenshots.clone();
            }
            if !l.tags.is_empty() && !text::looks_like_tag_ids(&l.tags) {
                result.tags = l.tags.clone();
            }
            if !l.creator.is_empty() && l.creator != "Unknown" {
                result.creator = l.creator.clone();
            }
            if !l.version.is_empty() {
                result.version = l.version.clone();
            }
            if l.rating > 0.0 {
                result.rating = l.rating;
            }
        }

        if let Some(ref h) = hint {
            if !h.title.is_empty() {
                result.title = h.title.clone();
            }
            if !h.screenshots.is_empty() {
                result.screenshots = h.screenshots.clone();
            }
            if !h.tags.is_empty() && !text::looks_like_tag_ids(&h.tags) {
                result.tags = h.tags.clone();
            }
            if !h.creator.is_empty() && h.creator != "Unknown" {
                result.creator = h.creator.clone();
            }
            if !h.version.is_empty() {
                result.version = h.version.clone();
            }
            if h.rating > 0.0 {
                result.rating = h.rating;
            }
        }

        if text::looks_like_tag_ids(&result.tags)
            && !thread_tags.is_empty()
            && !text::looks_like_tag_ids(&thread_tags)
        {
            result.tags = thread_tags;
        }

        // Cover + screenshots: SAM / match hint first, supplemented with thread CDN URLs.
        let api = hint.as_ref().or_else(|| list.as_ref());
        if let Some(api) = api {
            let mut screen_pool: Vec<String> = Vec::new();
            let mut push_unique = |url: String| {
                let url = text::upgrade_image_url(&url);
                if url.is_empty() || text::is_branding_image(&url) {
                    return;
                }
                if !screen_pool.iter().any(|u| u == &url) {
                    screen_pool.push(url);
                }
            };

            for url in &thread_all_images {
                let u = text::upgrade_image_url(url);
                if text::is_cdn_attachment(&u) && !text::is_xenforo_thumbnail(&u) {
                    push_unique(u);
                }
            }
            for s in &api.screenshots {
                if let Some(u) = text::sam_list_media_url(s) {
                    push_unique(u);
                }
            }
            if let Some(c) = text::sam_list_media_url(&api.cover) {
                push_unique(c);
            }
            for url in &thread_all_images {
                let u = text::upgrade_image_url(url);
                if u.contains("/attachments/")
                    && !text::is_xenforo_thumbnail(&u)
                    && !text::is_branding_image(&u)
                    && !text::is_cdn_attachment(&u)
                {
                    push_unique(text::attachment_page_url(&u));
                }
            }

            let cover_hint = text::sam_list_media_url(&api.cover)
                .unwrap_or_else(|| text::pick_best_cover("", &screen_pool));
            result.cover = text::pick_best_cover(&cover_hint, &screen_pool);
            result.screenshots = screen_pool
                .into_iter()
                .filter(|u| text::upgrade_image_url(u) != text::upgrade_image_url(&result.cover))
                .collect();
        }

        if result.cover.is_empty() {
            let (cover, _) = text::split_cover_and_screenshots(&thread_all_images);
            if !cover.is_empty() {
                result.cover = cover;
            }
        } else {
            let (thread_cover, _) = text::split_cover_and_screenshots(&thread_all_images);
            if !thread_cover.is_empty()
                && text::is_post_banner(&thread_cover)
                && !text::is_post_banner(&result.cover)
            {
                result.cover = thread_cover;
            }
        }

        if result.screenshots.is_empty() {
            let (_, shots) = text::split_cover_and_screenshots(&thread_all_images);
            result.screenshots = shots;
        }

        result.title = text::clean_f95_title(&result.title);
        result
    }

    fn normalize_version(value: &str) -> String {
        value
            .trim()
            .trim_start_matches(['v', 'V'])
            .trim()
            .to_lowercase()
    }

    fn version_update_available(stored: Option<&str>, latest: &str) -> bool {
        let latest_norm = Self::normalize_version(latest);
        if latest_norm.is_empty() {
            return false;
        }
        let Some(stored) = stored.filter(|s| !s.trim().is_empty()) else {
            return false;
        };
        Self::normalize_version(stored) != latest_norm
    }

    pub async fn check_game_version_update(&self, game_id: i64) -> AppResult<VersionCheckResult> {
        let game = self.db.get_game(game_id)?;
        let thread_id = game.f95_thread_id.ok_or_else(|| {
            AppError::BadRequest("This game is not linked to an F95Zone thread.".into())
        })?;

        let client = self.ensure_f95_client().await?;
        let thread = client.fetch_thread_metadata(thread_id).await?;
        let list_entry = client.fetch_list_entry(thread_id).await.ok().flatten();

        let mut latest_version = thread.result.version.clone();
        if let Some(ref entry) = list_entry {
            if !entry.version.trim().is_empty() {
                latest_version = entry.version.clone();
            }
        }

        let update_available =
            Self::version_update_available(game.version.as_deref(), &latest_version);

        Ok(VersionCheckResult {
            stored_version: game.version.clone(),
            latest_version,
            update_available,
            f95_url: game.f95_url,
        })
    }

    pub async fn match_archive(&self, req: MatchRequest) -> AppResult<Game> {
        if req.archive_id.is_none() && req.archive_path.is_none() {
            return Err(AppError::BadRequest(
                "archive_id or archive_path required".into(),
            ));
        }

        let client = self.ensure_f95_client().await?;

        let thread = client.fetch_thread_metadata(req.thread_id).await?;
        let description = thread.description.clone();
        let post_screenshots = thread.screenshots.clone();
        let list_entry = client.fetch_list_entry(req.thread_id).await.ok().flatten();
        let mut result = Self::merge_match_result(req.hint, list_entry, thread);

        if result.screenshots.is_empty() {
            result.screenshots = post_screenshots;
        }

        let game_id: i64 = if let Some(archive_id) = req.archive_id {
            self.db.get_platform_archive(archive_id)?.game_id
        } else {
            let path = req.archive_path.as_deref().unwrap();
            let archives = self.db.list_archives()?;
            archives
                .into_iter()
                .find(|a| a.path == path)
                .and_then(|a| a.game_id)
                .ok_or_else(|| AppError::NotFound("archive not found".into()))?
        };

        let cover_path = cache_thread_media(
            &self.db,
            &client,
            game_id,
            req.thread_id,
            &result.cover,
            &result.screenshots,
        )
        .await?;

        let game = self.db.apply_metadata_match(
            req.archive_id,
            req.archive_path.as_deref(),
            &result,
            cover_path,
            description,
        )?;

        let archive_id = if let Some(id) = req.archive_id {
            Some(id)
        } else if let Some(path) = req.archive_path.as_deref() {
            self.db
                .list_platform_archives(game_id)?
                .into_iter()
                .find(|a| a.path == path)
                .map(|a| a.id)
        } else {
            None
        };

        if let Some(archive_id) = archive_id {
            let platform = req
                .platform
                .as_deref()
                .and_then(crate::platform::normalize_platform)
                .filter(|p| p != "unknown")
                .unwrap_or_else(|| {
                    self.db
                        .get_platform_archive(archive_id)
                        .map(|a| a.platform)
                        .unwrap_or_else(|_| "unknown".into())
                });

            if platform != "unknown" {
                let _ = self.assign_archive_platform(archive_id, &platform, true)?;
                return self.db.get_game(game_id);
            }
        }

        Ok(game)
    }

    pub fn cover_api_url(&self, game: &Game) -> Option<String> {
        game.cover_image_path.as_ref().and_then(|path| {
            f95zone::cover_url_to_api_path(path, self.db.data_dir())
        })
    }
}

pub type SharedState = Arc<AppState>;
