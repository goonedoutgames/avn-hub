use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::f95zone::{self, text, F95Client, ThreadMetadata};
use crate::models::{
    F95SearchResult, GameDetail, GameSummary, LibraryFilter, ScreenshotItem, SettingsView,
    StorageStats, UpdateGameUserData, VersionCheckResult,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024; // 50 MiB

pub struct AppState {
    pub db: Database,
    f95: Mutex<Option<F95Client>>,
}

impl AppState {
    pub fn new(data_dir: impl AsRef<Path>) -> AppResult<Arc<Self>> {
        let db = Database::open(data_dir)?;
        Ok(Arc::new(Self {
            db,
            f95: Mutex::new(None),
        }))
    }

    pub fn max_attachment_bytes(&self) -> u64 {
        self.db
            .get_setting("max_attachment_bytes")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_ATTACHMENT_BYTES)
    }

    pub async fn ensure_f95_client(&self) -> AppResult<F95Client> {
        {
            let cached = {
                let guard = self.f95.lock().await;
                guard.clone()
            };
            if let Some(client) = cached {
                if client.probe_auth().await.unwrap_or(false) {
                    return Ok(client);
                }
            }
        }

        if let Some(cookies) = self.db.get_setting("f95_cookies")? {
            if !cookies.trim().is_empty() {
                let client = F95Client::from_cookies(&cookies)?;
                if client.probe_auth().await.unwrap_or(false) {
                    *self.f95.lock().await = Some(client.clone());
                    return Ok(client);
                }
            }
        }

        let username = self.db.get_setting("f95_username")?;
        let password = self.db.get_setting("f95_password")?;
        match (username, password) {
            (Some(user), Some(pass)) if !user.is_empty() && !pass.is_empty() => {
                let cookies = f95zone::auth::login(&user, &pass).await?;
                self.db.set_setting("f95_cookies", &cookies)?;
                let client = F95Client::from_cookies(&cookies)?;
                *self.f95.lock().await = Some(client.clone());
                Ok(client)
            }
            _ => Err(AppError::BadRequest(
                "F95Zone credentials not configured. Add them in Settings.".into(),
            )),
        }
    }

    pub async fn f95_login(&self, username: &str, password: &str) -> AppResult<String> {
        let cookies = f95zone::auth::login(username, password).await?;
        self.db.set_setting("f95_username", username)?;
        self.db.set_setting("f95_password", password)?;
        self.db.set_setting("f95_cookies", &cookies)?;
        let client = F95Client::from_cookies(&cookies)?;
        let ok = client.probe_auth().await?;
        *self.f95.lock().await = Some(client.clone());
        if ok {
            Ok("Logged in to F95Zone successfully.".into())
        } else {
            Err(AppError::BadRequest(
                "Login succeeded but F95Zone API probe failed.".into(),
            ))
        }
    }

    pub async fn f95_set_cookies(&self, cookies: &str) -> AppResult<String> {
        let client = F95Client::from_cookies(cookies)?;
        if !client.probe_auth().await? {
            return Err(AppError::BadRequest(
                "Cookies did not authenticate with F95Zone.".into(),
            ));
        }
        self.db.set_setting("f95_cookies", cookies)?;
        *self.f95.lock().await = Some(client.clone());
        Ok("F95Zone cookies saved.".into())
    }

    pub async fn settings_view(&self) -> AppResult<SettingsView> {
        let f95_authenticated = match self.ensure_f95_client().await {
            Ok(c) => c.probe_auth().await.unwrap_or(false),
            Err(_) => false,
        };
        Ok(SettingsView {
            data_dir: self.db.data_dir().display().to_string(),
            f95_username: self.db.get_setting("f95_username")?,
            f95_password_set: self
                .db
                .get_setting("f95_password")?
                .map(|p| !p.is_empty())
                .unwrap_or(false),
            f95_cookies_set: self
                .db
                .get_setting("f95_cookies")?
                .map(|c| !c.is_empty())
                .unwrap_or(false),
            f95_authenticated,
            app_password_set: self
                .db
                .get_setting("app_password_hash")?
                .map(|h| !h.is_empty())
                .unwrap_or(false),
            max_attachment_bytes: self.max_attachment_bytes(),
        })
    }

    pub async fn search_f95(
        &self,
        query: &str,
        page: u32,
        sort: &str,
    ) -> AppResult<Vec<F95SearchResult>> {
        self.catalog_search(f95zone::CatalogFilter {
            search: query.to_string(),
            page: page.max(1),
            sort: sort.to_string(),
            ..f95zone::CatalogFilter::default()
        })
        .await
    }

    pub async fn browse_f95(&self, page: u32, sort: &str) -> AppResult<Vec<F95SearchResult>> {
        self.catalog_search(f95zone::CatalogFilter {
            page: page.max(1),
            sort: sort.to_string(),
            ..f95zone::CatalogFilter::default()
        })
        .await
    }

    pub async fn catalog_search(
        &self,
        filter: f95zone::CatalogFilter,
    ) -> AppResult<Vec<F95SearchResult>> {
        let client = self.ensure_f95_client().await?;
        client.search_filtered(filter).await
    }

    pub async fn preview_thread(&self, input: &str) -> AppResult<F95SearchResult> {
        let thread_id = f95zone::parse_f95_thread_id(input)
            .ok_or_else(|| AppError::BadRequest("Invalid F95Zone thread URL or id".into()))?;
        let client = self.ensure_f95_client().await?;
        let merged = self.fetch_merged_metadata(&client, thread_id).await?;
        Ok(merged.result)
    }

    async fn fetch_merged_metadata(
        &self,
        client: &F95Client,
        thread_id: i64,
    ) -> AppResult<ThreadMetadata> {
        let thread = client.fetch_thread_metadata(thread_id).await?;
        let list_entry = client.fetch_list_entry(thread_id).await?;
        Ok(merge_match_result(thread, list_entry))
    }

    pub async fn add_game_from_f95(&self, input: &str) -> AppResult<GameDetail> {
        let thread_id = f95zone::parse_f95_thread_id(input)
            .ok_or_else(|| AppError::BadRequest("Invalid F95Zone thread URL or id".into()))?;

        if let Some(existing) = self.db.get_game_by_thread(thread_id)? {
            return self.game_detail(existing.id);
        }

        let client = self.ensure_f95_client().await?;
        let meta = self.fetch_merged_metadata(&client, thread_id).await?;
        let r = &meta.result;

        let id = self.db.insert_game_from_f95(
            &r.title,
            thread_id,
            &r.url,
            Some(r.version.as_str()).filter(|v| !v.is_empty()),
            Some(r.creator.as_str()).filter(|v| !v.is_empty()),
            &r.tags,
            meta.description.as_deref(),
            if r.rating > 0.0 { Some(r.rating) } else { None },
            None,
            None,
        )?;

        let cover = f95zone::cache_thread_media(
            &self.db,
            &client,
            id,
            thread_id,
            &r.cover,
            &meta.screenshots,
        )
        .await?;

        if let Some(path) = cover.as_deref() {
            self.db.set_cover_path(id, Some(path))?;
        }

        if let Ok(json) = serde_json::to_string(&r) {
            let _ = self
                .db
                .upsert_metadata_cache("f95zone", &thread_id.to_string(), Some(&r.title), &json);
        }

        self.game_detail(id)
    }

    pub async fn refresh_game_metadata(&self, game_id: i64) -> AppResult<GameDetail> {
        let game = self.db.get_game(game_id)?;
        let thread_id = game
            .f95_thread_id
            .ok_or_else(|| AppError::BadRequest("Game has no F95Zone thread".into()))?;

        let client = self.ensure_f95_client().await?;
        let meta = self.fetch_merged_metadata(&client, thread_id).await?;
        let r = &meta.result;

        let cover = f95zone::cache_thread_media(
            &self.db,
            &client,
            game_id,
            thread_id,
            &r.cover,
            &meta.screenshots,
        )
        .await?;

        self.db.update_game_metadata(
            game_id,
            &r.title,
            Some(r.version.as_str()).filter(|v| !v.is_empty()),
            Some(r.creator.as_str()).filter(|v| !v.is_empty()),
            &r.tags,
            meta.description.as_deref(),
            if r.rating > 0.0 { Some(r.rating) } else { None },
            None,
            cover.as_deref(),
            &r.url,
        )?;

        self.game_detail(game_id)
    }

    pub fn list_library(&self, filter: &LibraryFilter) -> AppResult<Vec<GameSummary>> {
        let games = self.db.list_games(filter)?;
        Ok(games
            .into_iter()
            .map(|game| {
                let cover_url = game
                    .cover_image_path
                    .as_deref()
                    .and_then(|p| f95zone::cover_url_to_api_path(p, self.db.data_dir()));
                GameSummary { game, cover_url }
            })
            .collect())
    }

    pub fn game_detail(&self, game_id: i64) -> AppResult<GameDetail> {
        let game = self.db.get_game(game_id)?;
        let media = self.db.list_game_media(game_id)?;
        let cover_full_url = media
            .iter()
            .find(|m| m.media_type == "cover")
            .map(|m| m.source_url.clone());

        let cover_url = game
            .cover_image_path
            .as_deref()
            .and_then(|p| f95zone::cover_url_to_api_path(p, self.db.data_dir()));

        let is_custom_cover = match (&game.cover_image_path, media.iter().find(|m| m.media_type == "cover")) {
            (Some(path), Some(cover)) => {
                cover.local_path.as_deref() != Some(path.as_str())
            }
            (Some(_), None) => true,
            _ => false,
        };

        let screenshots = media
            .iter()
            .filter(|m| m.media_type == "screenshot")
            .map(|m| ScreenshotItem {
                full_url: m.source_url.clone(),
                cached_url: m
                    .local_path
                    .as_deref()
                    .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir())),
            })
            .collect();

        Ok(GameDetail {
            game,
            cover_url,
            cover_full_url,
            screenshots,
            is_custom_cover,
            saves: self.db.list_saves(game_id)?,
            patches: self.db.list_patches(game_id)?,
        })
    }

    pub fn update_user_data(&self, game_id: i64, data: UpdateGameUserData) -> AppResult<GameDetail> {
        self.db.update_game_user_data(game_id, &data)?;
        self.game_detail(game_id)
    }

    pub fn set_cover_from_screenshot(&self, game_id: i64, index: usize) -> AppResult<GameDetail> {
        let media = self.db.list_game_media(game_id)?;
        let screens: Vec<_> = media
            .into_iter()
            .filter(|m| m.media_type == "screenshot")
            .collect();
        let shot = screens
            .get(index)
            .ok_or_else(|| AppError::BadRequest("Screenshot index out of range".into()))?;
        let path = shot
            .local_path
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("Screenshot has no local path".into()))?;
        self.db.set_cover_path(game_id, Some(path))?;
        self.game_detail(game_id)
    }

    pub fn reset_cover(&self, game_id: i64) -> AppResult<GameDetail> {
        let media = self.db.list_game_media(game_id)?;
        let cover = media.into_iter().find(|m| m.media_type == "cover");
        if let Some(path) = cover.and_then(|m| m.local_path) {
            self.db.set_cover_path(game_id, Some(&path))?;
        }
        self.game_detail(game_id)
    }

    pub async fn delete_game(&self, game_id: i64) -> AppResult<()> {
        let game = self.db.get_game(game_id)?;
        self.db.delete_game(game_id)?;
        if let Some(thread_id) = game.f95_thread_id {
            let media_dir = self.db.media_dir().join(thread_id.to_string());
            let _ = std::fs::remove_dir_all(media_dir);
        }
        let game_dir = self.db.game_dir(game_id);
        let _ = std::fs::remove_dir_all(game_dir);
        Ok(())
    }

    pub async fn check_version(&self, game_id: i64) -> AppResult<VersionCheckResult> {
        let game = self.db.get_game(game_id)?;
        let thread_id = game
            .f95_thread_id
            .ok_or_else(|| AppError::BadRequest("Game has no F95Zone thread".into()))?;
        let client = self.ensure_f95_client().await?;
        let meta = self.fetch_merged_metadata(&client, thread_id).await?;
        let latest = meta.result.version;
        let stored = game.version.clone();
        let update_available = versions_differ(stored.as_deref(), &latest);
        Ok(VersionCheckResult {
            game_id,
            stored_version: stored,
            latest_version: latest,
            update_available,
            f95_url: game.f95_url,
        })
    }

    pub async fn check_all_versions(&self) -> AppResult<Vec<VersionCheckResult>> {
        let games = self.db.list_games(&LibraryFilter::default())?;
        let mut results = Vec::new();
        for game in games {
            if game.f95_thread_id.is_none() {
                continue;
            }
            match self.check_version(game.id).await {
                Ok(r) => results.push(r),
                Err(e) => tracing::warn!("version check failed for game {}: {e}", game.id),
            }
        }
        Ok(results)
    }

    pub fn save_attachment_bytes(
        &self,
        game_id: i64,
        kind: AttachmentKind,
        filename: &str,
        bytes: &[u8],
        description: Option<&str>,
    ) -> AppResult<i64> {
        let _ = self.db.get_game(game_id)?;
        let max = self.max_attachment_bytes();
        if bytes.len() as u64 > max {
            return Err(AppError::BadRequest(format!(
                "File exceeds max size of {max} bytes"
            )));
        }

        let safe_name = sanitize_filename(filename);
        let sub = match kind {
            AttachmentKind::Save => "saves",
            AttachmentKind::Patch => "patches",
        };
        let dir = self.db.game_dir(game_id).join(sub);
        std::fs::create_dir_all(&dir)?;
        let path = unique_path(&dir, &safe_name);
        std::fs::write(&path, bytes)?;
        let size = bytes.len() as i64;
        let path_str = path.display().to_string();

        match kind {
            AttachmentKind::Save => self.db.insert_save(game_id, &path_str, &safe_name, size),
            AttachmentKind::Patch => {
                self.db
                    .insert_patch(game_id, &path_str, &safe_name, size, description)
            }
        }
    }

    pub fn delete_save(&self, save_id: i64) -> AppResult<()> {
        let save = self.db.delete_save(save_id)?;
        let _ = std::fs::remove_file(save.path);
        Ok(())
    }

    pub fn delete_patch(&self, patch_id: i64) -> AppResult<()> {
        let patch = self.db.delete_patch(patch_id)?;
        let _ = std::fs::remove_file(patch.path);
        Ok(())
    }

    pub fn resolve_media_file(&self, relative: &str) -> AppResult<PathBuf> {
        let cleaned = relative.trim_start_matches('/');
        if cleaned.contains("..") {
            return Err(AppError::BadRequest("Invalid media path".into()));
        }
        let path = self.db.media_dir().join(cleaned);
        if !path.exists() {
            return Err(AppError::NotFound("media not found".into()));
        }
        Ok(path)
    }

    pub fn storage_stats(&self) -> AppResult<StorageStats> {
        let data_dir = self.db.data_dir().to_path_buf();
        Ok(StorageStats {
            media_cache_bytes: dir_size(&data_dir.join("media")),
            saves_bytes: sum_named(&data_dir.join("games"), "saves"),
            patches_bytes: sum_named(&data_dir.join("games"), "patches"),
            database_bytes: std::fs::metadata(data_dir.join("avn-hub.db"))
                .map(|m| m.len())
                .unwrap_or(0),
            data_dir_bytes: dir_size(&data_dir),
            data_dir: data_dir.display().to_string(),
        })
    }

    pub fn purge_media_cache(&self) -> AppResult<()> {
        let media = self.db.media_dir();
        if media.exists() {
            std::fs::remove_dir_all(&media)?;
            std::fs::create_dir_all(&media)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AttachmentKind {
    Save,
    Patch,
}

fn merge_match_result(
    mut thread: ThreadMetadata,
    list: Option<F95SearchResult>,
) -> ThreadMetadata {
    let Some(sam) = list else {
        return thread;
    };

    if !sam.title.is_empty() {
        thread.result.title = sam.title;
    }
    if !sam.version.is_empty() {
        thread.result.version = sam.version;
    }
    if sam.rating > 0.0 {
        thread.result.rating = sam.rating;
    }
    if !sam.creator.is_empty() && sam.creator != "Unknown" {
        thread.result.creator = sam.creator;
    }
    if !sam.date.is_empty() {
        thread.result.date = sam.date;
    }
    if !text::looks_like_tag_ids(&sam.tags) && !sam.tags.is_empty() {
        thread.result.tags = sam.tags;
    }
    if thread.result.cover.is_empty() && !sam.cover.is_empty() {
        thread.result.cover = sam.cover.clone();
    }
    if thread.screenshots.is_empty() && !sam.screenshots.is_empty() {
        thread.screenshots = sam.screenshots.clone();
        thread.result.screenshots = sam.screenshots;
    }
    thread
}

fn versions_differ(stored: Option<&str>, latest: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_start_matches(['v', 'V'])
            .trim()
            .to_lowercase()
    };
    match stored {
        Some(s) if !s.is_empty() && !latest.is_empty() => norm(s) != norm(latest),
        _ => false,
    }
}

fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned
    }
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for i in 1..10_000 {
        let p = dir.join(format!("{stem}_{i}{ext}"));
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{stem}_{}{ext}", uuid::Uuid::new_v4()))
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walk_size(path)
}

fn walk_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            total += walk_size(&p);
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

fn sum_named(games_root: &Path, folder: &str) -> u64 {
    if !games_root.exists() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(games_root) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        total += dir_size(&entry.path().join(folder));
    }
    total
}
