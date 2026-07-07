use crate::error::{AppError, AppResult};
use crate::models::{ArchiveEntry, F95SearchResult, Game, GameMediaRecord, Settings};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
}

mod attachments;

impl Database {
    pub fn new(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("avn-hub.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(include_str!("schema.sql"))?;
        let db = Self {
            conn: Mutex::new(conn),
            data_dir: data_dir.to_path_buf(),
        };
        db.ensure_defaults()?;
        db.migrate_attachments()?;
        Ok(db)
    }

    fn ensure_defaults(&self) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let archive_path: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'archive_path'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        if archive_path.is_none() {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('archive_path', '')",
                [],
            )?;
        }
        let _ = conn.execute(
            "ALTER TABLE tus_uploads ADD COLUMN replace_game_id INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN play_status TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN user_rating REAL",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN user_notes TEXT",
            [],
        );
        let _ = conn.execute(
            "UPDATE games SET play_status = 'unplayed' WHERE play_status IS NULL",
            [],
        );
        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    pub fn media_dir(&self) -> PathBuf {
        self.data_dir.join("media")
    }

    fn now() -> String {
        Utc::now().to_rfc3339()
    }

    pub fn get_settings(&self) -> AppResult<Settings> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let archive_path: String = conn.query_row(
            "SELECT value FROM settings WHERE key = 'archive_path'",
            [],
            |row| row.get(0),
        )?;
        let f95_username = Self::get_setting(&conn, "f95_username")?;
        let f95_password = Self::get_setting(&conn, "f95_password")?;
        let f95_cookies = Self::get_setting(&conn, "f95_cookies")?;

        let f95_authenticated = f95_cookies
            .as_ref()
            .is_some_and(|c| !c.trim().is_empty());

        let http_auth_username = Self::get_setting(&conn, "http_auth_username")?;
        let http_auth_hash = Self::get_setting(&conn, "http_auth_password_hash")?;
        let http_auth_configured =
            http_auth_hash.as_ref().is_some_and(|h| !h.trim().is_empty());

        Ok(Settings {
            archive_path,
            data_dir: self.data_dir.display().to_string(),
            f95_username,
            f95_password_set: f95_password.is_some_and(|p| !p.is_empty()),
            f95_cookies,
            f95_authenticated,
            http_auth_configured,
            http_auth_username,
        })
    }

    fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(AppError::from)
    }

    fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn update_settings(
        &self,
        archive_path: Option<String>,
        f95_username: Option<String>,
        f95_password: Option<String>,
        f95_cookies: Option<String>,
        http_auth_username: Option<String>,
        http_auth_password: Option<String>,
        http_auth_remove: Option<bool>,
    ) -> AppResult<Settings> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        if let Some(path) = archive_path {
            Self::set_setting(&conn, "archive_path", &path)?;
        }
        if let Some(username) = f95_username {
            Self::set_setting(&conn, "f95_username", &username)?;
        }
        if let Some(password) = f95_password {
            Self::set_setting(&conn, "f95_password", &password)?;
        }
        if let Some(cookies) = f95_cookies {
            Self::set_setting(&conn, "f95_cookies", &cookies)?;
        }
        if http_auth_remove == Some(true) {
            conn.execute(
                "DELETE FROM settings WHERE key IN ('http_auth_username', 'http_auth_password_hash')",
                [],
            )?;
            conn.execute("DELETE FROM sessions", [])?;
        } else {
            if let Some(username) = http_auth_username {
                Self::set_setting(&conn, "http_auth_username", &username)?;
            }
            if let Some(password) = http_auth_password {
                let hash = crate::http_auth::hash_password(&password)?;
                Self::set_setting(&conn, "http_auth_password_hash", &hash)?;
            }
        }
        drop(conn);
        self.get_settings()
    }

    pub fn http_auth_configured(&self) -> AppResult<bool> {
        Ok(self.get_settings()?.http_auth_configured)
    }

    pub fn verify_http_credentials(&self, username: &str, password: &str) -> AppResult<bool> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let stored_user = Self::get_setting(&conn, "http_auth_username")?
            .unwrap_or_default();
        let stored_hash = Self::get_setting(&conn, "http_auth_password_hash")?
            .unwrap_or_default();
        if stored_user.is_empty() || stored_hash.is_empty() {
            return Ok(false);
        }
        if stored_user != username {
            return Ok(false);
        }
        crate::http_auth::verify_password(password, &stored_hash)
    }

    pub fn create_session(&self) -> AppResult<String> {
        self.purge_expired_sessions()?;
        let token = uuid::Uuid::new_v4().to_string();
        let expires = chrono::Utc::now() + chrono::Duration::days(7);
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "INSERT INTO sessions (token, expires_at) VALUES (?1, ?2)",
            params![token, expires.to_rfc3339()],
        )?;
        Ok(token)
    }

    pub fn session_valid(&self, token: &str) -> AppResult<bool> {
        if token.trim().is_empty() {
            return Ok(false);
        }
        self.purge_expired_sessions()?;
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let found: Option<String> = conn
            .query_row(
                "SELECT token FROM sessions WHERE token = ?1",
                params![token],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn delete_session(&self, token: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    pub fn purge_expired_sessions(&self) -> AppResult<()> {
        let now = Self::now();
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "DELETE FROM sessions WHERE expires_at < ?1",
            params![now],
        )?;
        Ok(())
    }

    pub fn uploads_dir(&self) -> PathBuf {
        self.data_dir.join("uploads")
    }

    pub fn create_tus_upload(
        &self,
        id: &str,
        filename: &str,
        size: i64,
        replace_game_id: Option<i64>,
        upload_kind: &str,
        platform: Option<&str>,
        replace_archive_id: Option<i64>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "INSERT INTO tus_uploads (id, filename, size, offset, replace_game_id, upload_kind, platform, replace_archive_id, created_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                filename,
                size,
                replace_game_id,
                upload_kind,
                platform,
                replace_archive_id,
                Self::now()
            ],
        )?;
        Ok(())
    }

    pub fn get_tus_upload(
        &self,
        id: &str,
    ) -> AppResult<
        Option<(
            String,
            i64,
            i64,
            Option<i64>,
            String,
            Option<String>,
            Option<i64>,
        )>,
    > {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn
            .query_row(
                "SELECT filename, size, offset, replace_game_id, upload_kind, platform, replace_archive_id
                 FROM tus_uploads WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn update_tus_offset(&self, id: &str, offset: i64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE tus_uploads SET offset = ?1 WHERE id = ?2",
            params![offset, id],
        )?;
        Ok(())
    }

    pub fn delete_tus_upload(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute("DELETE FROM tus_uploads WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn upsert_archive_file(
        &self,
        full_path: &str,
        filename: &str,
        size: i64,
    ) -> AppResult<()> {
        let _ = self.upsert_archive(full_path, filename, size)?;
        Ok(())
    }

    pub fn replace_game_archive(
        &self,
        game_id: i64,
        new_path: &str,
        new_filename: &str,
        new_size: i64,
        platform: Option<&str>,
    ) -> AppResult<Game> {
        let platform = platform
            .and_then(crate::platform::normalize_platform)
            .unwrap_or_else(|| {
                crate::platform::detect_platform_from_filename(new_filename).to_string()
            });

        let archives = self.list_platform_archives(game_id)?;
        if let Some(existing) = archives.iter().find(|a| a.platform == platform) {
            let _ = self.replace_platform_archive(existing.id, new_path, new_filename, new_size)?;
        } else if let Some(default) = archives.iter().find(|a| a.is_default) {
            let _ = self.replace_platform_archive(default.id, new_path, new_filename, new_size)?;
        } else {
            let _ = self.insert_platform_archive(
                game_id,
                &platform,
                new_path,
                new_filename,
                new_size,
                true,
                Some(&Self::now()),
            )?;
        }
        self.get_game(game_id)
    }

    pub fn delete_game_archive(&self, game_id: i64) -> AppResult<()> {
        let game = self.get_game(game_id)?;
        let archives = self.list_platform_archives(game_id)?;

        for archive in &archives {
            if std::path::Path::new(&archive.path).exists() {
                let _ = std::fs::remove_file(&archive.path);
            }
        }

        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "DELETE FROM game_platform_archives WHERE game_id = ?1",
            params![game_id],
        )?;
        conn.execute("DELETE FROM game_saves WHERE game_id = ?1", params![game_id])?;
        conn.execute("DELETE FROM game_patches WHERE game_id = ?1", params![game_id])?;
        conn.execute("DELETE FROM media WHERE game_id = ?1", params![game_id])?;
        conn.execute("DELETE FROM games WHERE id = ?1", params![game_id])?;
        drop(conn);

        if let Some(tid) = game.f95_thread_id {
            let _ = std::fs::remove_dir_all(self.media_dir().join(tid.to_string()));
        }

        Ok(())
    }

    pub fn update_f95_cookies(&self, cookies: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        Self::set_setting(&conn, "f95_cookies", cookies)?;
        Ok(())
    }

    pub fn get_f95_credentials(&self) -> AppResult<(Option<String>, Option<String>)> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let username = Self::get_setting(&conn, "f95_username")?;
        let password = Self::get_setting(&conn, "f95_password")?;
        Ok((username, password))
    }

    pub fn clear_game_media(&self, game_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute("DELETE FROM media WHERE game_id = ?1", params![game_id])?;
        Ok(())
    }

    pub fn insert_media(
        &self,
        game_id: i64,
        url: &str,
        local_path: &str,
        media_type: &str,
    ) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "INSERT INTO media (game_id, url, local_path, media_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![game_id, url, local_path, media_type, Self::now()],
        )?;
        Ok(())
    }

    pub fn list_game_media(&self, game_id: i64) -> AppResult<Vec<GameMediaRecord>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT media_type, url, local_path FROM media WHERE game_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![game_id], |row| {
            Ok(GameMediaRecord {
                media_type: row.get(0)?,
                source_url: row.get(1)?,
                local_path: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_games(
        &self,
        name_search: Option<&str>,
        tag_filter: Option<&str>,
        tag_mode: Option<&str>,
        play_status_filter: Option<&str>,
        min_f95_rating: Option<f64>,
        max_f95_rating: Option<f64>,
        min_user_rating: Option<f64>,
        max_user_rating: Option<f64>,
        sort: Option<&str>,
    ) -> AppResult<Vec<Game>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, archive_path, archive_filename, archive_size,
                    f95_thread_id, f95_url, version, developer, tags, description,
                    cover_image_path, rating, status, play_status, user_rating, user_notes,
                    matched, created_at, updated_at
             FROM games ORDER BY title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], Self::row_to_game)?;
        let mut games = rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)?;

        if let Some(q) = name_search.filter(|s| !s.trim().is_empty()) {
            let terms = parse_search_terms(q);
            if !terms.is_empty() {
                games.retain(|game| game_matches_name_search(game, &terms));
            }
        }

        if let Some(tags) = tag_filter.filter(|s| !s.trim().is_empty()) {
            let terms = parse_tag_filter_terms(tags);
            if !terms.is_empty() {
                let mode = TagFilterMode::from_query(tag_mode);
                games.retain(|game| game_matches_tag_filter(game, &terms, mode));
            }
        }

        if let Some(statuses) = play_status_filter.filter(|s| !s.trim().is_empty()) {
            let terms: Vec<String> = statuses
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if !terms.is_empty() {
                games.retain(|game| {
                    let status = effective_play_status(game);
                    terms.iter().any(|t| t == status)
                });
            }
        }

        if min_f95_rating.is_some() || max_f95_rating.is_some() {
            games.retain(|game| {
                rating_in_range(game.rating, min_f95_rating, max_f95_rating)
            });
        }

        if min_user_rating.is_some() || max_user_rating.is_some() {
            games.retain(|game| {
                rating_in_range(game.user_rating, min_user_rating, max_user_rating)
            });
        }

        sort_games(&mut games, sort);
        Ok(games)
    }

    pub fn list_matched_tags(&self) -> AppResult<Vec<crate::models::LibraryTag>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT tags FROM games WHERE matched = 1",
        )?;
        let rows = stmt.query_map([], |row| {
            let tags_json: String = row.get(0)?;
            Ok(tags_json)
        })?;

        let mut counts: std::collections::BTreeMap<String, (String, usize)> =
            std::collections::BTreeMap::new();
        for tags_json in rows.flatten() {
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            for tag in tags {
                let key = tag.to_lowercase();
                counts
                    .entry(key)
                    .and_modify(|(_, count)| *count += 1)
                    .or_insert((tag, 1));
            }
        }

        let mut out: Vec<crate::models::LibraryTag> = counts
            .into_values()
            .map(|(tag, count)| crate::models::LibraryTag { tag, count })
            .collect();
        out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
        Ok(out)
    }

    pub fn get_game(&self, id: i64) -> AppResult<Game> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.query_row(
            "SELECT id, title, archive_path, archive_filename, archive_size,
                    f95_thread_id, f95_url, version, developer, tags, description,
                    cover_image_path, rating, status, play_status, user_rating, user_notes,
                    matched, created_at, updated_at
             FROM games WHERE id = ?1",
            params![id],
            Self::row_to_game,
        )
        .map_err(|_| AppError::NotFound(format!("game {id} not found")))
    }

    fn row_to_game(row: &rusqlite::Row<'_>) -> rusqlite::Result<Game> {
        let tags_json: String = row.get(9)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        Ok(Game {
            id: row.get(0)?,
            title: row.get(1)?,
            archive_path: row.get(2)?,
            archive_filename: row.get(3)?,
            archive_size: row.get(4)?,
            f95_thread_id: row.get(5)?,
            f95_url: row.get(6)?,
            version: row.get(7)?,
            developer: row.get(8)?,
            tags,
            description: row.get(10)?,
            cover_image_path: row.get(11)?,
            rating: row.get(12)?,
            status: row.get(13)?,
            play_status: row
                .get::<_, Option<String>>(14)?
                .or_else(|| Some("unplayed".into())),
            user_rating: row.get(15)?,
            user_notes: row.get(16)?,
            matched: row.get::<_, i64>(17)? != 0,
            created_at: row.get(18)?,
            updated_at: row.get(19)?,
        })
    }

    pub fn list_archives(&self) -> AppResult<Vec<ArchiveEntry>> {
        self.list_archives_from_platform_table()
    }

    pub fn upsert_archive(
        &self,
        path: &str,
        filename: &str,
        size: i64,
    ) -> AppResult<(bool, i64)> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();

        let existing_platform: Option<(i64, i64)> = conn
            .query_row(
                "SELECT game_id, size FROM game_platform_archives WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((game_id, old_size)) = existing_platform {
            if old_size != size {
                conn.execute(
                    "UPDATE game_platform_archives SET size = ?1, updated_at = ?2 WHERE path = ?3",
                    params![size, now, path],
                )?;
                drop(conn);
                self.sync_game_default_archive(game_id)?;
            }
            return Ok((false, game_id));
        }

        let existing: Option<(i64, i64)> = conn
            .query_row(
                "SELECT id, archive_size FROM games WHERE archive_path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((id, old_size)) = existing {
            if old_size != size {
                conn.execute(
                    "UPDATE games SET archive_size = ?1, updated_at = ?2 WHERE id = ?3",
                    params![size, now, id],
                )?;
            }
            drop(conn);
            let platform = crate::platform::detect_platform_from_filename(filename);
            let _ = self.insert_platform_archive(id, platform, path, filename, size, true, None)?;
            return Ok((false, id));
        }

        let title = filename
            .rsplit_once('.')
            .map(|(name, _)| name.to_string())
            .unwrap_or_else(|| filename.to_string());

        conn.execute(
            "INSERT INTO games (title, archive_path, archive_filename, archive_size,
                                play_status, matched, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'unplayed', 0, ?5, ?5)",
            params![title, path, filename, size, now],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        let platform = crate::platform::detect_platform_from_filename(filename);
        let _ = self.insert_platform_archive(id, platform, path, filename, size, true, None)?;
        Ok((true, id))
    }

    pub fn apply_metadata_match(
        &self,
        archive_id: Option<i64>,
        archive_path: Option<&str>,
        result: &F95SearchResult,
        cover_path: Option<String>,
        description: Option<String>,
    ) -> AppResult<Game> {
        let (game_id, path) = if let Some(id) = archive_id {
            let archive = self.get_platform_archive(id)?;
            (archive.game_id, archive.path)
        } else if let Some(path) = archive_path {
            let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
            let game_id: i64 = conn
                .query_row(
                    "SELECT game_id FROM game_platform_archives WHERE path = ?1",
                    params![path],
                    |row| row.get(0),
                )
                .or_else(|_| {
                    conn.query_row(
                        "SELECT id FROM games WHERE archive_path = ?1",
                        params![path],
                        |row| row.get(0),
                    )
                })
                .map_err(|_| AppError::NotFound("archive not found".into()))?;
            drop(conn);
            (game_id, path.to_string())
        } else {
            return Err(AppError::BadRequest(
                "archive_id or archive_path required".into(),
            ));
        };

        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();
        let tags_json = serde_json::to_string(&result.tags)?;

        let updated = conn.execute(
            "UPDATE games SET
                title = ?1, f95_thread_id = ?2, f95_url = ?3, version = ?4,
                developer = ?5, tags = ?6, cover_image_path = ?7, rating = ?8,
                description = ?9, matched = 1, updated_at = ?10
             WHERE id = ?11",
            params![
                result.title,
                result.thread_id,
                result.url,
                result.version,
                result.creator,
                tags_json,
                cover_path,
                result.rating,
                description,
                now,
                game_id,
            ],
        )?;

        if updated == 0 {
            return Err(AppError::NotFound(format!("game not found for archive: {path}")));
        }

        conn.execute(
            "INSERT INTO metadata_cache (source, external_id, title, data, fetched_at)
             VALUES ('f95zone', ?1, ?2, ?3, ?4)
             ON CONFLICT(source, external_id) DO UPDATE SET
                title = excluded.title, data = excluded.data, fetched_at = excluded.fetched_at",
            params![
                result.thread_id.to_string(),
                result.title,
                serde_json::to_string(result)?,
                now,
            ],
        )?;

        drop(conn);
        self.get_game(game_id)
    }

    pub fn unmatch_archive(&self, game_id: i64) -> AppResult<Game> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();

        let (archive_filename, thread_id): (String, Option<i64>) = conn.query_row(
            "SELECT archive_filename, f95_thread_id FROM games WHERE id = ?1",
            params![game_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::NotFound(format!("game {game_id} not found")))?;

        let fallback_title = archive_filename
            .rsplit_once('.')
            .map(|(n, _)| n.to_string())
            .unwrap_or(archive_filename);

        conn.execute(
            "UPDATE games SET
                title = ?1, f95_thread_id = NULL, f95_url = NULL, version = NULL,
                developer = NULL, tags = '[]', description = NULL, cover_image_path = NULL,
                rating = NULL, status = NULL, matched = 0, updated_at = ?2
             WHERE id = ?3",
            params![fallback_title, now, game_id],
        )?;

        conn.execute("DELETE FROM media WHERE game_id = ?1", params![game_id])?;
        drop(conn);

        if let Some(tid) = thread_id {
            let _ = std::fs::remove_dir_all(self.media_dir().join(tid.to_string()));
        }

        self.get_game(game_id)
    }

    pub fn purge_media_cache(&self) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute("DELETE FROM media", [])?;
        conn.execute(
            "UPDATE games SET cover_image_path = NULL, updated_at = ?1",
            params![Self::now()],
        )?;
        drop(conn);

        let media_dir = self.media_dir();
        if media_dir.exists() {
            for entry in std::fs::read_dir(&media_dir)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        Ok(())
    }

    pub fn set_game_cover(&self, game_id: i64, screenshot_index: usize) -> AppResult<Game> {
        let screenshot_paths: Vec<String> = self
            .list_game_media(game_id)?
            .into_iter()
            .filter(|m| m.media_type == "screenshot")
            .filter_map(|m| m.local_path)
            .collect();

        let cover_path = screenshot_paths.get(screenshot_index).ok_or_else(|| {
            AppError::BadRequest(format!("screenshot index {screenshot_index} out of range"))
        })?;

        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE games SET cover_image_path = ?1, updated_at = ?2 WHERE id = ?3",
            params![cover_path, Self::now(), game_id],
        )?;
        drop(conn);
        self.get_game(game_id)
    }

    pub fn reset_game_cover(&self, game_id: i64) -> AppResult<Game> {
        let default_cover = self
            .list_game_media(game_id)?
            .into_iter()
            .find(|m| m.media_type == "cover")
            .and_then(|m| m.local_path);

        let cover_path = default_cover.ok_or_else(|| {
            AppError::BadRequest("No default cover found for this game".into())
        })?;

        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE games SET cover_image_path = ?1, updated_at = ?2 WHERE id = ?3",
            params![cover_path, Self::now(), game_id],
        )?;
        drop(conn);
        self.get_game(game_id)
    }

    pub fn update_game_user_data(
        &self,
        game_id: i64,
        play_status: Option<&str>,
        user_rating: Option<f64>,
        user_notes: Option<&str>,
    ) -> AppResult<Game> {
        if let Some(status) = play_status {
            let allowed = ["unplayed", "playing", "completed", "dropped"];
            if !allowed.contains(&status) {
                return Err(AppError::BadRequest(format!(
                    "invalid play_status: {status}"
                )));
            }
        }

        if let Some(rating) = user_rating {
            if !(0.0..=5.0).contains(&rating) {
                return Err(AppError::BadRequest(
                    "user_rating must be between 0 and 5".into(),
                ));
            }
        }

        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE games SET play_status = ?1, user_rating = ?2, user_notes = ?3, updated_at = ?4 WHERE id = ?5",
            params![
                play_status,
                user_rating,
                user_notes,
                Self::now(),
                game_id
            ],
        )?;
        drop(conn);
        self.get_game(game_id)
    }

    pub fn sum_archive_sizes(&self) -> AppResult<i64> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.query_row(
            "SELECT COALESCE(SUM(archive_size), 0) FROM games",
            [],
            |row| row.get(0),
        )
        .map_err(AppError::from)
    }

    pub fn cache_metadata(&self, source: &str, result: &F95SearchResult) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "INSERT INTO metadata_cache (source, external_id, title, data, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source, external_id) DO UPDATE SET
                title = excluded.title, data = excluded.data, fetched_at = excluded.fetched_at",
            params![
                source,
                result.thread_id.to_string(),
                result.title,
                serde_json::to_string(result)?,
                Self::now(),
            ],
        )?;
        Ok(())
    }
}

pub fn default_data_dir() -> PathBuf {
    std::env::var("AVN_HUB_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("avn-hub")
        })
}

fn effective_play_status(game: &Game) -> &str {
    game.play_status
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("unplayed")
}

fn play_status_rank(status: &str) -> i32 {
    match status {
        "unplayed" => 0,
        "playing" => 1,
        "completed" => 2,
        "dropped" => 3,
        _ => 99,
    }
}

fn rating_in_range(value: Option<f64>, min: Option<f64>, max: Option<f64>) -> bool {
    let Some(value) = value else {
        return false;
    };
    if let Some(min) = min {
        if value < min {
            return false;
        }
    }
    if let Some(max) = max {
        if value > max {
            return false;
        }
    }
    true
}

fn compare_option_f64(a: Option<f64>, b: Option<f64>, ascending: bool) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => {
            if ascending {
                a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
            }
        }
    }
}

fn sort_games(games: &mut Vec<Game>, sort: Option<&str>) {
    match sort.unwrap_or("title").trim().to_lowercase().as_str() {
        "title_desc" => {
            games.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase()));
        }
        "f95_rating" => {
            games.sort_by(|a, b| compare_option_f64(a.rating, b.rating, false));
        }
        "f95_rating_asc" => {
            games.sort_by(|a, b| compare_option_f64(a.rating, b.rating, true));
        }
        "user_rating" => {
            games.sort_by(|a, b| compare_option_f64(a.user_rating, b.user_rating, false));
        }
        "user_rating_asc" => {
            games.sort_by(|a, b| compare_option_f64(a.user_rating, b.user_rating, true));
        }
        "play_status" => {
            games.sort_by(|a, b| {
                play_status_rank(effective_play_status(a))
                    .cmp(&play_status_rank(effective_play_status(b)))
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            });
        }
        "play_status_desc" => {
            games.sort_by(|a, b| {
                play_status_rank(effective_play_status(b))
                    .cmp(&play_status_rank(effective_play_status(a)))
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            });
        }
        _ => {
            games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        }
    }
}

fn parse_search_terms(query: &str) -> Vec<String> {
    query
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_tag_filter_terms(query: &str) -> Vec<String> {
    query
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagFilterMode {
    And,
    Or,
}

impl TagFilterMode {
    fn from_query(mode: Option<&str>) -> Self {
        match mode.map(|s| s.trim().to_lowercase()).as_deref() {
            Some("or") => Self::Or,
            _ => Self::And,
        }
    }
}

fn normalize_search_text(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn game_matches_name_search(game: &Game, terms: &[String]) -> bool {
    let title = normalize_search_text(&game.title);
    let filename = normalize_search_text(&game.archive_filename);
    let developer = game
        .developer
        .as_deref()
        .map(normalize_search_text)
        .unwrap_or_default();

    terms.iter().all(|term| {
        let term = normalize_search_text(term);
        if term.is_empty() {
            return true;
        }
        title.contains(&term) || filename.contains(&term) || developer.contains(&term)
    })
}

fn game_matches_tag_filter(game: &Game, terms: &[String], mode: TagFilterMode) -> bool {
    if terms.is_empty() {
        return true;
    }

    let tags: Vec<String> = game
        .tags
        .iter()
        .map(|tag| normalize_search_text(tag))
        .collect();

    let matches_term = |term: &String| {
        let term = normalize_search_text(term);
        if term.is_empty() {
            return true;
        }
        tags.iter().any(|tag| tag_contains_term(tag, &term))
    };

    match mode {
        TagFilterMode::And => terms.iter().all(matches_term),
        TagFilterMode::Or => terms.iter().any(matches_term),
    }
}

fn tag_contains_term(tag: &str, term: &str) -> bool {
    tag == term || tag.contains(term) || term.contains(tag)
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn sample_game(tags: &[&str]) -> Game {
        Game {
            id: 1,
            title: "Actual Roommates 2".into(),
            archive_path: "/games/ar2.zip".into(),
            archive_filename: "ar2.zip".into(),
            archive_size: 1,
            f95_thread_id: Some(1),
            f95_url: None,
            version: None,
            developer: Some("HanakoXVN".into()),
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            description: None,
            cover_image_path: None,
            rating: None,
            status: None,
            play_status: Some("unplayed".into()),
            user_rating: None,
            user_notes: None,
            matched: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn matches_single_tag() {
        let game = sample_game(&["lesbian", "harem"]);
        assert!(game_matches_tag_filter(&game, &["lesbian".into()], TagFilterMode::And));
        assert!(!game_matches_tag_filter(&game, &["rpg".into()], TagFilterMode::And));
        assert!(!game_matches_name_search(&game, &["lesbian".into()]));
    }

    #[test]
    fn matches_multiple_terms_with_and() {
        let game = sample_game(&["lesbian", "harem", "romance"]);
        assert!(game_matches_tag_filter(
            &game,
            &["lesbian".into(), "harem".into()],
            TagFilterMode::And
        ));
        assert!(!game_matches_tag_filter(
            &game,
            &["lesbian".into(), "incest".into()],
            TagFilterMode::And
        ));
    }

    #[test]
    fn matches_multiple_terms_with_or() {
        let game = sample_game(&["lesbian", "harem"]);
        assert!(game_matches_tag_filter(
            &game,
            &["lesbian".into(), "incest".into()],
            TagFilterMode::Or
        ));
        assert!(!game_matches_tag_filter(
            &game,
            &["incest".into(), "rpg".into()],
            TagFilterMode::Or
        ));
    }

    #[test]
    fn matches_title_and_developer() {
        let game = sample_game(&["3dcg"]);
        assert!(game_matches_name_search(&game, &["roommates".into()]));
        assert!(game_matches_name_search(&game, &["hanako".into()]));
    }
}
