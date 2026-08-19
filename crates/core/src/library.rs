use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::f95zone::{self, text, F95Client, TagCatalog, ThreadMetadata};
use crate::models::{
    CatalogPage, CatalogPreview, CatalogTag, DownloadLink, F95SearchResult, GameDetail, GameSummary,
    LibraryFilter, PlaySessionDto, PlaytimeSummary, ScreenshotItem, SettingsView, StorageStats,
    UpdateGameUserData, VersionCheckResult,
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
        // Hot path: never probe F95 here. probe_auth() is a full SAM round-trip and
        // routinely hangs long enough for Cloudflare/SWAG to return 502 on refresh/add.
        // Trust the in-memory client, then stored cookies; validate only in Settings.
        {
            let guard = self.f95.lock().await;
            if let Some(client) = guard.as_ref() {
                return Ok(client.clone());
            }
        }

        if let Some(cookies) = self.db.get_setting("f95_cookies")? {
            if !cookies.trim().is_empty() {
                let client = F95Client::from_cookies(&cookies)?;
                *self.f95.lock().await = Some(client.clone());
                return Ok(client);
            }
        }

        let username = self.db.get_setting("f95_username")?;
        let password = self.db.get_setting("f95_password")?;
        match (username, password) {
            (Some(user), Some(pass)) if !user.is_empty() && !pass.is_empty() => {
                let cookies = match tokio::time::timeout(
                    std::time::Duration::from_secs(20),
                    f95zone::auth::login(&user, &pass),
                )
                .await
                {
                    Ok(result) => result?,
                    Err(_) => {
                        return Err(AppError::Other(
                            "Timed out logging in to F95Zone. Try cookie login in Settings.".into(),
                        ));
                    }
                };
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

    /// Cookie header string for desktop WebView session seeding (masked download links).
    pub fn f95_cookies_export(&self) -> AppResult<Option<String>> {
        Ok(self.db.get_setting("f95_cookies")?)
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
            tag_click_action: self
                .db
                .get_setting("tag_click_action")?
                .filter(|v| v == "library" || v == "browse")
                .unwrap_or_else(|| "library".into()),
            save_sync_enabled: self
                .db
                .get_setting("save_sync_enabled")?
                .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
            save_sync_max_per_game: self
                .db
                .get_setting("save_sync_max_per_game")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            save_sync_rolling: self
                .db
                .get_setting("save_sync_rolling")?
                .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true),
            save_sync_name_format: self
                .db
                .get_setting("save_sync_name_format")?
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "auto_{timestamp}".into()),
        })
    }

    pub fn set_save_sync_settings(
        &self,
        enabled: Option<bool>,
        max_per_game: Option<i64>,
        rolling: Option<bool>,
        name_format: Option<&str>,
    ) -> AppResult<()> {
        if let Some(v) = enabled {
            self.db
                .set_setting("save_sync_enabled", if v { "1" } else { "0" })?;
        }
        if let Some(v) = max_per_game {
            let clamped = v.clamp(1, 100);
            self.db
                .set_setting("save_sync_max_per_game", &clamped.to_string())?;
        }
        if let Some(v) = rolling {
            self.db
                .set_setting("save_sync_rolling", if v { "1" } else { "0" })?;
        }
        if let Some(fmt) = name_format {
            let trimmed = fmt.trim();
            if !trimmed.is_empty() {
                self.db.set_setting("save_sync_name_format", trimmed)?;
            }
        }
        Ok(())
    }

    pub fn set_tag_click_action(&self, action: &str) -> AppResult<()> {
        let normalized = match action.trim().to_lowercase().as_str() {
            "browse" => "browse",
            _ => "library",
        };
        self.db.set_setting("tag_click_action", normalized)
    }

    pub async fn search_f95(
        &self,
        query: &str,
        page: u32,
        sort: &str,
    ) -> AppResult<CatalogPage> {
        self.catalog_search(f95zone::CatalogFilter {
            search: query.to_string(),
            page: page.max(1),
            sort: sort.to_string(),
            ..f95zone::CatalogFilter::default()
        })
        .await
    }

    pub async fn browse_f95(&self, page: u32, sort: &str) -> AppResult<CatalogPage> {
        self.catalog_search(f95zone::CatalogFilter {
            page: page.max(1),
            sort: sort.to_string(),
            ..f95zone::CatalogFilter::default()
        })
        .await
    }

    fn tag_catalog(&self) -> TagCatalog {
        let mut catalog = TagCatalog::seed().clone();
        if let Ok(Some(raw)) = self.db.get_setting("f95_tag_map") {
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<i64, String>>(&raw) {
                catalog.merge_from_id_map(&map);
            }
        }
        catalog
    }

    /// Fetch / refresh F95 `cmd=options` tag map when missing or older than 24h.
    async fn ensure_tag_map(&self) -> AppResult<()> {
        let stale = match self.db.get_setting("f95_tag_map_fetched_at") {
            Ok(Some(ts)) => match ts.parse::<i64>() {
                Ok(secs) => {
                    let now = chrono::Utc::now().timestamp();
                    now.saturating_sub(secs) > 24 * 60 * 60
                }
                Err(_) => true,
            },
            _ => true,
        };
        let missing = self
            .db
            .get_setting("f95_tag_map")
            .ok()
            .flatten()
            .is_none();
        if !stale && !missing {
            return Ok(());
        }

        let Ok(client) = self.ensure_f95_client().await else {
            return Ok(());
        };
        match tokio::time::timeout(
            std::time::Duration::from_secs(8),
            client.fetch_tag_options(),
        )
        .await
        {
            Ok(Ok(Some(map))) => {
                if let Ok(json) = serde_json::to_string(&map) {
                    let _ = self.db.set_setting("f95_tag_map", &json);
                    let _ = self.db.set_setting(
                        "f95_tag_map_fetched_at",
                        &chrono::Utc::now().timestamp().to_string(),
                    );
                }
            }
            Ok(Ok(None)) => {
                tracing::debug!("F95 tag options returned empty; keeping cached map");
            }
            Ok(Err(e)) => {
                tracing::debug!(error = %e, "F95 tag options fetch failed");
            }
            Err(_) => {
                tracing::warn!("F95 tag options fetch timed out");
            }
        }
        Ok(())
    }

    pub async fn catalog_search(
        &self,
        mut filter: f95zone::CatalogFilter,
    ) -> AppResult<CatalogPage> {
        let _ = self.ensure_tag_map().await;
        let catalog = self.tag_catalog();
        // Resolve names → F95 numeric IDs before hitting SAM (names are ignored by F95).
        filter.tags = catalog.resolve_query_list(&filter.tags).map_err(|unknown| {
            AppError::BadRequest(format!(
                "Unknown F95 tag(s): {}. Pick a tag from the Browse list.",
                unknown.join(", ")
            ))
        })?;
        filter.notags = catalog
            .resolve_query_list(&filter.notags)
            .map_err(|unknown| {
                AppError::BadRequest(format!(
                    "Unknown F95 exclude tag(s): {}.",
                    unknown.join(", ")
                ))
            })?;

        // SAM treats rows < 30 as 30; allow up to 90.
        filter.rows = filter.rows.clamp(30, 90);

        tracing::info!(
            search = %filter.search,
            creator = %filter.creator,
            page = filter.page,
            rows = filter.rows,
            sort = %filter.sort,
            date_days = filter.date_days,
            tag_mode = %filter.tag_mode,
            tags = ?filter.tags,
            notags = ?filter.notags,
            prefixes = ?filter.prefixes,
            "catalog search request"
        );

        let client = self.ensure_f95_client().await.map_err(|e| {
            tracing::warn!(error = %e, "catalog search: F95 client unavailable");
            e
        })?;
        let mut page = client.search_filtered(filter).await?;
        // Map numeric SAM ids → names using seed + live options map.
        for result in &mut page.items {
            result.tags = catalog.labels_for_ids(&result.tags);
        }
        // Never scrape threads during browse — that N+1 HTML fetch times out the API
        // and risks F95 rate limits. Platforms come from library / prior add-refresh cache only.
        self.hydrate_catalog_platforms(&mut page.items);
        self.hydrate_catalog_library(&mut page.items);
        tracing::info!(
            hits = page.items.len(),
            page = page.page,
            total_pages = page.total_pages,
            "catalog search response"
        );
        Ok(CatalogPage {
            has_more: page.has_more(),
            items: page.items,
            page: page.page,
            total_pages: page.total_pages,
            rows: page.rows,
        })
    }

    /// Attach platforms already known from the library or metadata cache.
    /// Does not fetch F95 thread HTML (browse must stay a single SAM request).
    fn hydrate_catalog_platforms(&self, results: &mut [F95SearchResult]) {
        let thread_ids: Vec<i64> = results.iter().map(|r| r.thread_id).collect();
        let from_library = self.db.platforms_by_thread_ids(&thread_ids).unwrap_or_default();

        for result in results.iter_mut() {
            if !result.platforms.is_empty() {
                continue;
            }
            if let Some(platforms) = from_library.get(&result.thread_id) {
                result.platforms = platforms.clone();
                continue;
            }
            if let Ok(Some(json)) = self
                .db
                .get_metadata_cache("f95zone", &result.thread_id.to_string())
            {
                if let Ok(cached) = serde_json::from_str::<F95SearchResult>(&json) {
                    if !cached.platforms.is_empty() {
                        result.platforms = cached.platforms;
                        continue;
                    }
                }
            }
            if let Ok(Some(json)) = self
                .db
                .get_metadata_cache("f95zone_platforms", &result.thread_id.to_string())
            {
                if let Ok(platforms) = serde_json::from_str::<Vec<String>>(&json) {
                    if !platforms.is_empty() {
                        result.platforms = platforms;
                    }
                }
            }
        }
    }

    fn hydrate_catalog_library(&self, results: &mut [F95SearchResult]) {
        let thread_ids: Vec<i64> = results.iter().map(|r| r.thread_id).collect();
        let ids = self.db.game_ids_by_thread_ids(&thread_ids).unwrap_or_default();
        for result in results.iter_mut() {
            if let Some(game_id) = ids.get(&result.thread_id) {
                result.in_library = true;
                result.library_game_id = Some(*game_id);
            }
        }
    }

    fn cache_platforms(&self, thread_id: i64, platforms: &[String]) -> AppResult<()> {
        if platforms.is_empty() {
            return Ok(());
        }
        let json = serde_json::to_string(platforms)?;
        self.db
            .upsert_metadata_cache("f95zone_platforms", &thread_id.to_string(), None, &json)
    }

    pub async fn catalog_tags(&self, query: Option<&str>, limit: usize) -> AppResult<Vec<CatalogTag>> {
        let _ = self.ensure_tag_map().await;
        let catalog = self.tag_catalog();

        let limit = limit.clamp(1, 2000);
        let rows = match query.map(str::trim).filter(|s| !s.is_empty()) {
            Some(q) => catalog.search(q, limit),
            None => {
                let mut all = catalog.all_sorted();
                all.truncate(limit);
                all
            }
        };
        Ok(rows
            .into_iter()
            .map(|(id, name)| CatalogTag { id, name })
            .collect())
    }

    pub async fn preview_thread(
        &self,
        input: &str,
        title_hint: Option<&str>,
    ) -> AppResult<CatalogPreview> {
        let input = input.trim();
        let thread_id = f95zone::parse_f95_thread_id(input)
            .ok_or_else(|| AppError::BadRequest("Invalid F95Zone thread URL or id".into()))?;
        let hint = resolve_title_hint(title_hint, input);
        tracing::info!(%input, thread_id, hint = %hint, "catalog preview started");

        let client = self.ensure_f95_client().await.map_err(|e| {
            tracing::warn!(thread_id, error = %e, "catalog preview: F95 client unavailable");
            e
        })?;

        // Same parallel strategy as add — never block forever on thread HTML parse.
        let merged = self
            .fetch_preview_or_add_metadata(&client, thread_id, &hint, "catalog preview")
            .await?;

        // Tag map refresh is best-effort and must not stall the preview response.
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.ensure_tag_map(),
        )
        .await;

        let catalog = self.tag_catalog();
        let mut result = merged.result;
        result.tags = catalog.labels_for_ids(&result.tags);
        // Prefer the fuller OP gallery when SAM returned fewer screenshots.
        if merged.screenshots.len() > result.screenshots.len() {
            result.screenshots = merged.screenshots.clone();
        }

        if !result.platforms.is_empty() {
            let _ = self.cache_platforms(thread_id, &result.platforms);
        }

        let (in_library, library_game_id) = match self.db.get_game_by_thread(thread_id)? {
            Some(g) => (true, Some(g.id)),
            None => (false, None),
        };
        result.in_library = in_library;
        result.library_game_id = library_game_id;

        tracing::info!(
            thread_id,
            title = %result.title,
            screenshots = result.screenshots.len(),
            has_description = merged.description.as_ref().is_some_and(|d| !d.trim().is_empty()),
            in_library,
            "catalog preview ready"
        );

        Ok(CatalogPreview {
            result,
            description: merged.description,
            in_library,
            library_game_id,
        })
    }

    /// Shared SAM + thread scrape for preview/add. Always finishes within ~15s wall clock.
    ///
    /// Numeric SAM id search often misses; pass a catalog title or URL-derived `title_hint`.
    async fn fetch_preview_or_add_metadata(
        &self,
        client: &F95Client,
        thread_id: i64,
        title_hint: &str,
        context: &str,
    ) -> AppResult<ThreadMetadata> {
        tracing::info!(
            thread_id,
            hint = %title_hint,
            %context,
            "F95 metadata fetch starting (SAM ∥ thread)"
        );
        let sam_secs = if title_hint.trim().chars().count() >= 3 {
            12
        } else {
            8
        };
        let (list_res, thread_res) = tokio::join!(
            tokio::time::timeout(
                std::time::Duration::from_secs(sam_secs),
                client.fetch_list_entry_with_hint(thread_id, title_hint),
            ),
            tokio::time::timeout(
                std::time::Duration::from_secs(14),
                client.fetch_thread_metadata(thread_id),
            ),
        );

        let list_entry = match list_res {
            Ok(Ok(entry)) => {
                tracing::info!(
                    thread_id,
                    found = entry.is_some(),
                    %context,
                    "SAM lookup finished"
                );
                entry
            }
            Ok(Err(e)) => {
                tracing::warn!(thread_id, error = %e, %context, "SAM lookup failed");
                None
            }
            Err(_) => {
                tracing::warn!(thread_id, %context, "SAM lookup timed out");
                None
            }
        };
        let thread = match thread_res {
            Ok(Ok(meta)) => {
                tracing::info!(
                    thread_id,
                    title = %meta.result.title,
                    screenshots = meta.screenshots.len(),
                    %context,
                    "thread scrape finished"
                );
                Some(meta)
            }
            Ok(Err(e)) => {
                tracing::warn!(thread_id, error = %e, %context, "thread scrape failed");
                None
            }
            Err(_) => {
                tracing::warn!(thread_id, %context, "thread scrape timed out");
                None
            }
        };

        match (thread, list_entry) {
            (Some(t), list) => Ok(merge_match_result(t, list)),
            (None, Some(sam)) => {
                tracing::info!(thread_id, %context, "using SAM-only metadata (thread unavailable)");
                Ok(f95zone::ThreadMetadata {
                    screenshots: sam.screenshots.clone(),
                    all_images: Vec::new(),
                    description: None,
                    result: sam,
                })
            }
            (None, None) => {
                tracing::warn!(
                    thread_id,
                    hint = %title_hint,
                    %context,
                    "no SAM or thread metadata"
                );
                let hint_note = if title_hint.trim().is_empty() {
                    " Try adding from Browse (sends the title) or paste the full F95 thread URL."
                } else {
                    ""
                };
                Err(AppError::Other(format!(
                    "Could not reach F95Zone for this thread (SAM miss + thread scrape timeout/fail). \
                     Check F95 login or cookies in Settings.{hint_note}"
                )))
            }
        }
    }

    async fn fetch_merged_metadata(
        &self,
        client: &F95Client,
        thread_id: i64,
        title_hint: &str,
    ) -> AppResult<ThreadMetadata> {
        // Kept for refresh/version paths — spawn_blocking-safe thread fetch.
        match tokio::time::timeout(
            std::time::Duration::from_secs(18),
            self.fetch_merged_metadata_inner(client, thread_id, title_hint),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(AppError::Other(
                "Timed out fetching F95Zone metadata. Try again in a moment.".into(),
            )),
        }
    }

    async fn fetch_merged_metadata_inner(
        &self,
        client: &F95Client,
        thread_id: i64,
        title_hint: &str,
    ) -> AppResult<ThreadMetadata> {
        self.fetch_preview_or_add_metadata(client, thread_id, title_hint, "metadata merge")
            .await
    }

    pub async fn add_game_from_f95(
        &self,
        input: &str,
        title_hint: Option<&str>,
    ) -> AppResult<GameDetail> {
        let input = input.trim();
        let hint = resolve_title_hint(title_hint, input);
        tracing::info!(%input, hint = %hint, "library add started");
        let thread_id = f95zone::parse_f95_thread_id(input)
            .ok_or_else(|| AppError::BadRequest("Invalid F95Zone thread URL or id".into()))?;

        if let Some(existing) = self.db.get_game_by_thread(thread_id)? {
            tracing::info!(thread_id, game_id = existing.id, "library add: already in library");
            return self.game_detail(existing.id);
        }

        tracing::info!(thread_id, "library add: ensuring F95 client");
        let client = self.ensure_f95_client().await.map_err(|e| {
            tracing::warn!(thread_id, error = %e, "library add: F95 client unavailable");
            e
        })?;

        let meta = self
            .fetch_preview_or_add_metadata(&client, thread_id, &hint, "library add")
            .await?;

        let r = &meta.result;
        if r.title.trim().is_empty() {
            tracing::warn!(thread_id, "library add: empty title from F95");
            return Err(AppError::Other(
                "F95Zone returned no title for this thread.".into(),
            ));
        }

        tracing::info!(thread_id, title = %r.title, "library add: inserting game row");
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.ensure_tag_map(),
        )
        .await;
        let tags = {
            let resolved = self.tag_catalog().labels_for_ids(&r.tags);
            if !resolved.is_empty() {
                resolved
            } else if !text::looks_like_tag_ids(&r.tags) {
                r.tags.clone()
            } else {
                // Keep unresolved IDs only as last resort (UI hides pure digits).
                Vec::new()
            }
        };
        let id = self.db.insert_game_from_f95(
            &r.title,
            thread_id,
            &r.url,
            Some(r.version.as_str()).filter(|v| !v.is_empty()),
            f95zone::normalize_creator(&r.creator).as_deref(),
            &tags,
            &r.platforms,
            meta.description.as_deref(),
            if r.rating > 0.0 { Some(r.rating) } else { None },
            None,
            None,
        )?;

        // Best-effort cover only — 4s max, never fail the add.
        let cover = match tokio::time::timeout(
            std::time::Duration::from_secs(4),
            f95zone::cache_thread_cover(
                &self.db,
                &client,
                id,
                thread_id,
                &r.cover,
                &meta.screenshots,
            ),
        )
        .await
        {
            Ok(Ok(path)) => path,
            Ok(Err(e)) => {
                tracing::debug!(game_id = id, error = %e, "cover cache failed while adding");
                None
            }
            Err(_) => {
                tracing::debug!(game_id = id, "cover cache timed out while adding");
                None
            }
        };

        if let Some(path) = cover.as_deref() {
            let _ = self.db.set_cover_path(id, Some(path));
        }

        // Immediate URL stubs so the gallery lists something; bytes land in the background.
        self.register_screenshot_urls(id, &meta.screenshots);
        self.schedule_screenshot_cache(id, thread_id, r.cover.clone(), meta.screenshots.clone());

        if let Ok(json) = serde_json::to_string(&r) {
            let _ = self
                .db
                .upsert_metadata_cache("f95zone", &thread_id.to_string(), Some(&r.title), &json);
        }
        if !r.platforms.is_empty() {
            let _ = self.cache_platforms(thread_id, &r.platforms);
        }

        tracing::info!(game_id = id, thread_id, title = %r.title, "library add succeeded");
        self.game_detail(id)
    }

    pub async fn refresh_game_metadata(&self, game_id: i64) -> AppResult<GameDetail> {
        let game = self.db.get_game(game_id)?;
        let thread_id = game
            .f95_thread_id
            .ok_or_else(|| AppError::BadRequest("Game has no F95Zone thread".into()))?;
        tracing::info!(game_id, thread_id, title = %game.title, "library refresh started");

        let client = self.ensure_f95_client().await.map_err(|e| {
            tracing::warn!(game_id, thread_id, error = %e, "library refresh: F95 client unavailable");
            e
        })?;

        // Same strategy as add: thread HTML is authoritative (SAM often misses by numeric id).
        // Platform UTF-8 slicing is fixed, so scraping is safe again — no media downloads.
        let title_hint = game
            .source_title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| game.title.clone());

        let (list_res, thread_res) = tokio::join!(
            tokio::time::timeout(
                std::time::Duration::from_secs(12),
                client.fetch_list_entry_with_hint(thread_id, &title_hint),
            ),
            tokio::time::timeout(
                std::time::Duration::from_secs(14),
                client.fetch_thread_metadata(thread_id),
            ),
        );

        let list_entry = match list_res {
            Ok(Ok(entry)) => entry,
            Ok(Err(e)) => {
                tracing::warn!(thread_id, error = %e, "SAM lookup failed during refresh");
                None
            }
            Err(_) => {
                tracing::warn!(thread_id, "SAM lookup timed out during refresh");
                None
            }
        };
        let thread = match thread_res {
            Ok(Ok(meta)) => Some(meta),
            Ok(Err(e)) => {
                tracing::warn!(thread_id, error = %e, "thread scrape failed during refresh");
                None
            }
            Err(_) => {
                tracing::warn!(thread_id, "thread scrape timed out during refresh");
                None
            }
        };

        let meta = match (thread, list_entry) {
            (Some(t), list) => merge_match_result(t, list),
            (None, Some(mut sam)) => {
                if sam.creator.is_empty() || sam.creator.eq_ignore_ascii_case("unknown") {
                    if let Some(dev) = game.developer.clone() {
                        sam.creator = dev;
                    }
                }
                f95zone::ThreadMetadata {
                    screenshots: sam.screenshots.clone(),
                    all_images: Vec::new(),
                    description: game.description.clone(),
                    result: sam,
                }
            }
            (None, None) => {
                tracing::warn!(game_id, thread_id, "library refresh: no SAM or thread metadata");
                return Err(AppError::Other(
                    "Could not reach F95Zone for this thread. Check F95 login in Settings and try again."
                        .into(),
                ));
            }
        };

        let r = &meta.result;
        let platforms = if r.platforms.is_empty() {
            game.platforms.clone()
        } else {
            r.platforms.clone()
        };
        let description = meta
            .description
            .clone()
            .filter(|d| !d.trim().is_empty())
            .or_else(|| game.description.clone());

        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.ensure_tag_map(),
        )
        .await;
        let tags = {
            let resolved = self.tag_catalog().labels_for_ids(&r.tags);
            if !resolved.is_empty() {
                resolved
            } else if !text::looks_like_tag_ids(&r.tags) {
                r.tags.clone()
            } else {
                // Don't wipe existing names with unresolved ids — caller may refresh later.
                if text::looks_like_tag_ids(&r.tags) && !game.tags.is_empty() {
                    game.tags.clone()
                } else {
                    Vec::new()
                }
            }
        };

        self.db.update_game_metadata(
            game_id,
            &r.title,
            Some(r.version.as_str()).filter(|v| !v.is_empty()),
            f95zone::normalize_creator(&r.creator).as_deref(),
            &tags,
            &platforms,
            description.as_deref(),
            if r.rating > 0.0 { Some(r.rating) } else { None },
            None,
            None, // keep existing cover
            &r.url,
        )?;

        if !platforms.is_empty() {
            let _ = self.cache_platforms(thread_id, &platforms);
        }

        if let Ok(json) = serde_json::to_string(&r) {
            let _ = self
                .db
                .upsert_metadata_cache("f95zone", &thread_id.to_string(), Some(&r.title), &json);
        }

        let shots = if meta.screenshots.is_empty() {
            r.screenshots.clone()
        } else {
            meta.screenshots.clone()
        };
        // Stubs first so the response always lists F95 URLs (web/desktop can display immediately).
        self.register_screenshot_urls(game_id, &shots);

        // Refresh is explicit — wait for hub-side cache so clients get `/api/v1/media/...`
        // without depending on a background race. Cap so we never hang forever.
        if !shots.is_empty() {
            match tokio::time::timeout(
                std::time::Duration::from_secs(120),
                f95zone::cache_thread_screenshots(
                    &self.db,
                    &client,
                    game_id,
                    thread_id,
                    &r.cover,
                    &shots,
                ),
            )
            .await
            {
                Ok(Ok(n)) => tracing::info!(game_id, n, "refresh cached screenshot gallery"),
                Ok(Err(e)) => tracing::warn!(game_id, error = %e, "refresh screenshot cache failed"),
                Err(_) => {
                    tracing::warn!(game_id, "refresh screenshot cache timed out — stubs remain");
                    self.schedule_screenshot_cache(game_id, thread_id, r.cover.clone(), shots);
                }
            }
        }

        tracing::info!(game_id, thread_id, title = %r.title, "library refresh succeeded");
        self.game_detail(game_id)
    }

    pub fn list_library(&self, filter: &LibraryFilter) -> AppResult<Vec<GameSummary>> {
        let games = self.db.list_games(filter)?;
        Ok(games
            .into_iter()
            .map(|mut game| {
                game.playtime_seconds = self.db.total_playtime_secs(game.id).unwrap_or(0);
                let cover_url = game
                    .cover_image_path
                    .as_deref()
                    .and_then(|p| f95zone::cover_url_to_api_path(p, self.db.data_dir()));
                let preview_urls = self.library_preview_urls(game.id, cover_url.as_deref());
                GameSummary {
                    game,
                    cover_url,
                    preview_urls,
                }
            })
            .collect())
    }

    fn library_preview_urls(&self, game_id: i64, cover_url: Option<&str>) -> Vec<String> {
        let Ok(media) = self.db.list_game_media(game_id) else {
            return cover_url.map(|u| vec![u.to_string()]).unwrap_or_default();
        };
        let mut urls = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let push = |urls: &mut Vec<String>, seen: &mut std::collections::HashSet<String>, url: String| {
            let key = url.to_lowercase();
            if url.is_empty() || !seen.insert(key) {
                return;
            }
            urls.push(url);
        };

        if let Some(cover) = cover_url {
            push(&mut urls, &mut seen, cover.to_string());
        }

        for m in media {
            let local = m
                .local_path
                .as_deref()
                .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir()));
            if let Some(url) = local {
                push(&mut urls, &mut seen, url);
            }
        }

        // Cap hover gallery size for list performance.
        urls.truncate(12);
        urls
    }

    pub fn game_detail(&self, game_id: i64) -> AppResult<GameDetail> {
        let mut game = self.db.get_game(game_id)?;
        game.playtime_seconds = self.db.total_playtime_secs(game_id).unwrap_or(0);
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

        let mut screenshots: Vec<ScreenshotItem> = media
            .iter()
            .filter(|m| m.media_type == "screenshot")
            .filter(|m| !m.source_url.trim().is_empty())
            .map(|m| ScreenshotItem {
                full_url: m.source_url.clone(),
                cached_url: m
                    .local_path
                    .as_deref()
                    .filter(|p| !p.trim().is_empty())
                    .and_then(|p| f95zone::media_url_to_api_path(p, self.db.data_dir())),
            })
            .collect();

        // Prefer hub-cached media; fall back to / merge F95 URLs from metadata.
        if screenshots.is_empty() {
            if let Some(thread_id) = game.f95_thread_id {
                for url in self.cached_screenshot_urls(thread_id) {
                    screenshots.push(ScreenshotItem {
                        full_url: url,
                        cached_url: None,
                    });
                }
            }
        } else if let Some(thread_id) = game.f95_thread_id {
            // Always merge metadata URLs missing from media rows (partial cache must
            // not hide the rest of the thread gallery).
            let existing: std::collections::HashSet<_> = screenshots
                .iter()
                .map(|s| s.full_url.to_lowercase())
                .collect();
            for url in self.cached_screenshot_urls(thread_id) {
                if existing.contains(&url.to_lowercase()) {
                    continue;
                }
                screenshots.push(ScreenshotItem {
                    full_url: url,
                    cached_url: None,
                });
            }
        }

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

    fn cached_screenshot_urls(&self, thread_id: i64) -> Vec<String> {
        let Ok(Some(json)) = self.db.get_metadata_cache("f95zone", &thread_id.to_string()) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
            return Vec::new();
        };
        let Some(arr) = value.get("screenshots").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        arr.iter()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                f95zone::text::download_media_url(s)
                    .or_else(|| f95zone::text::sam_list_media_url(s))
                    .unwrap_or_else(|| s.to_string())
            })
            .take(f95zone::MAX_THREAD_IMAGES)
            .collect()
    }

    /// Persist screenshot source URLs immediately so galleries list while bytes download.
    fn register_screenshot_urls(&self, game_id: i64, screenshots: &[String]) {
        if screenshots.is_empty() {
            return;
        }
        let Ok(media) = self.db.list_game_media(game_id) else {
            return;
        };

        // Keep on-disk paths keyed by source URL so refresh can expand the list
        // without discarding already-cached bytes.
        let cached_by_url: std::collections::HashMap<String, String> = media
            .iter()
            .filter(|m| m.media_type == "screenshot")
            .filter_map(|m| {
                let path = m.local_path.as_deref().filter(|p| !p.trim().is_empty())?;
                if !std::path::Path::new(path).is_file() {
                    return None;
                }
                Some((
                    f95zone::text::upgrade_image_url(&m.source_url).to_lowercase(),
                    path.to_string(),
                ))
            })
            .collect();

        let _ = self.db.clear_game_screenshot_media(game_id);
        let mut seen = std::collections::HashSet::new();
        for url in screenshots.iter().take(f95zone::MAX_THREAD_IMAGES) {
            let resolved = f95zone::text::download_media_url(url)
                .or_else(|| f95zone::text::sam_list_media_url(url))
                .unwrap_or_else(|| url.clone());
            let resolved = f95zone::text::upgrade_image_url(&resolved);
            if resolved.trim().is_empty() {
                continue;
            }
            let key = resolved.to_lowercase();
            if !seen.insert(key.clone()) {
                continue;
            }
            let local = cached_by_url.get(&key).map(String::as_str).unwrap_or("");
            let _ = self.db.insert_media(game_id, &resolved, local, "screenshot");
        }
    }

    /// Download screenshot bytes into hub media so clients use `/api/v1/media/...` (no F95).
    fn schedule_screenshot_cache(
        &self,
        game_id: i64,
        thread_id: i64,
        cover_url: String,
        screenshots: Vec<String>,
    ) {
        if screenshots.is_empty() {
            return;
        }
        let data_dir = self.db.data_dir().to_path_buf();
        let cookies = self.db.get_setting("f95_cookies").ok().flatten();
        tokio::spawn(async move {
            let Ok(db) = Database::open(&data_dir) else {
                tracing::warn!(game_id, "screenshot cache: could not open database");
                return;
            };
            // Prefer stored cookies; attachments CDN often works without, so still try.
            let client = match cookies
                .filter(|c| !c.trim().is_empty())
                .and_then(|c| F95Client::from_cookies(&c).ok())
            {
                Some(c) => c,
                None => match F95Client::from_cookies("") {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(game_id, error = %e, "screenshot cache: no F95 client");
                        return;
                    }
                },
            };
            match f95zone::cache_thread_screenshots(
                &db,
                &client,
                game_id,
                thread_id,
                &cover_url,
                &screenshots,
            )
            .await
            {
                Ok(n) => tracing::info!(game_id, n, "cached screenshot gallery on hub"),
                Err(e) => tracing::warn!(game_id, error = %e, "screenshot gallery cache failed"),
            }
        });
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
        let title_hint = game
            .source_title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| game.title.clone());
        let meta = self
            .fetch_merged_metadata(&client, thread_id, &title_hint)
            .await?;
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
            AttachmentKind::Save => {
                let id = self.db.insert_save(game_id, &path_str, &safe_name, size)?;
                self.enforce_save_retention(game_id)?;
                Ok(id)
            }
            AttachmentKind::Patch => {
                self.db
                    .insert_patch(game_id, &path_str, &safe_name, size, description)
            }
        }
    }

    fn enforce_save_retention(&self, game_id: i64) -> AppResult<()> {
        let enabled = self
            .db
            .get_setting("save_sync_enabled")?
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        if !enabled {
            return Ok(());
        }
        let rolling = self
            .db
            .get_setting("save_sync_rolling")?
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        if !rolling {
            return Ok(());
        }
        let max = self
            .db
            .get_setting("save_sync_max_per_game")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(10)
            .clamp(1, 100);
        for save in self.db.trim_saves_beyond(game_id, max)? {
            let _ = std::fs::remove_file(save.path);
        }
        Ok(())
    }

    pub fn playtime_summary(&self, game_id: i64) -> AppResult<PlaytimeSummary> {
        let _ = self.db.get_game(game_id)?;
        let total_seconds = self.db.total_playtime_secs(game_id)?;
        let sessions = self
            .db
            .list_play_sessions(game_id, 50)?
            .into_iter()
            .map(|(client_session_id, started_at, ended_at, duration_secs, client_id)| {
                PlaySessionDto {
                    client_session_id,
                    started_at,
                    ended_at,
                    duration_secs,
                    client_id,
                }
            })
            .collect();
        Ok(PlaytimeSummary {
            total_seconds,
            sessions,
        })
    }

    pub fn ingest_play_sessions(
        &self,
        game_id: i64,
        sessions: &[PlaySessionDto],
        synced_from: Option<&str>,
    ) -> AppResult<PlaytimeSummary> {
        let _ = self.db.get_game(game_id)?;
        for s in sessions {
            if s.client_session_id.trim().is_empty() || s.duration_secs < 0 {
                continue;
            }
            self.db.upsert_play_session(
                game_id,
                &s.client_session_id,
                &s.started_at,
                &s.ended_at,
                s.duration_secs,
                s.client_id.as_deref(),
                synced_from,
            )?;
        }
        self.playtime_summary(game_id)
    }

    pub async fn download_links_for_game(&self, game_id: i64) -> AppResult<Vec<DownloadLink>> {
        let game = self.db.get_game(game_id)?;
        let thread_id = game
            .f95_thread_id
            .ok_or_else(|| AppError::BadRequest("Game has no F95Zone thread".into()))?;
        let client = self.ensure_f95_client().await?;
        let html = client.fetch_thread_html(thread_id).await?;
        Ok(f95zone::extract_download_links(&html))
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
        let cleaned = relative
            .trim_start_matches('/')
            .replace('\\', "/");
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
        // Prefer SAM weighted rating when present; keep scraped value otherwise.
        thread.result.rating = sam.rating;
    }
    if let Some(creator) = f95zone::normalize_creator(&sam.creator) {
        thread.result.creator = creator;
    } else if let Some(creator) = f95zone::normalize_creator(&thread.result.creator) {
        thread.result.creator = creator;
    }
    if !sam.date.is_empty() {
        thread.result.date = sam.date;
    }
    if sam.likes.is_some() {
        thread.result.likes = sam.likes;
    }
    if sam.views.is_some() {
        thread.result.views = sam.views;
    }
    if !sam.prefixes.is_empty() && thread.result.prefixes.is_empty() {
        thread.result.prefixes = sam.prefixes;
    }
    // Prefer human-readable thread tags; otherwise keep SAM tags (usually numeric IDs).
    // Old logic skipped SAM IDs entirely — SAM-only adds then stored digits that UIs hide.
    thread.result.tags = prefer_tags(thread.result.tags, sam.tags);
    if thread.result.cover.is_empty() && !sam.cover.is_empty() {
        thread.result.cover = sam.cover.clone();
    }
    if thread.screenshots.is_empty() && !sam.screenshots.is_empty() {
        thread.screenshots = sam.screenshots.clone();
        thread.result.screenshots = sam.screenshots;
    }
    // Platforms come from thread overview scrape; SAM has none.
    if thread.result.platforms.is_empty() && !sam.platforms.is_empty() {
        thread.result.platforms = sam.platforms;
    }
    thread
}

fn prefer_tags(thread_tags: Vec<String>, sam_tags: Vec<String>) -> Vec<String> {
    let thread_human =
        !thread_tags.is_empty() && !text::looks_like_tag_ids(&thread_tags);
    if thread_human {
        thread_tags
    } else if !sam_tags.is_empty() {
        sam_tags
    } else {
        thread_tags
    }
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

/// Prefer an explicit catalog title; otherwise derive a hint from a thread URL slug.
fn resolve_title_hint(explicit: Option<&str>, input: &str) -> String {
    if let Some(hint) = explicit.map(str::trim).filter(|s| s.chars().count() >= 3) {
        return hint.to_string();
    }
    f95zone::parse_f95_thread_slug(input)
        .map(|slug| text::title_hint_from_thread_slug(&slug))
        .filter(|s| s.chars().count() >= 3)
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::prefer_tags;

    #[test]
    fn prefer_tags_keeps_thread_names_over_sam_ids() {
        let out = prefer_tags(
            vec!["harem".into(), "fantasy".into()],
            vec!["179".into(), "42".into()],
        );
        assert_eq!(out, vec!["harem", "fantasy"]);
    }

    #[test]
    fn prefer_tags_uses_sam_when_thread_empty() {
        let out = prefer_tags(vec![], vec!["179".into(), "392".into()]);
        assert_eq!(out, vec!["179", "392"]);
    }
}
