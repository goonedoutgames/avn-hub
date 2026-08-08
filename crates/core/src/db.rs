use crate::error::{AppError, AppResult};
use crate::models::{
    Game, GameMediaRecord, GamePatch, GameSave, LibraryFilter, LibraryPlatform, LibrarySort,
    LibraryTag, TagMode, UpdateGameUserData,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub struct Database {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
}

impl Database {
    pub fn open(data_dir: impl AsRef<Path>) -> AppResult<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(data_dir.join("media"))?;
        std::fs::create_dir_all(data_dir.join("games"))?;

        let db_path = data_dir.join("avn-hub.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Self::migrate(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            data_dir,
        })
    }

    fn migrate(conn: &Connection) -> AppResult<()> {
        // Existing DBs created before these columns existed.
        let _ = conn.execute("ALTER TABLE games ADD COLUMN source_title TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN title_custom INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN platforms TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        conn.execute(
            "UPDATE games SET source_title = title
             WHERE source_title IS NULL OR TRIM(source_title) = ''",
            [],
        )?;
        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn media_dir(&self) -> PathBuf {
        self.data_dir.join("media")
    }

    pub fn game_dir(&self, game_id: i64) -> PathBuf {
        self.data_dir.join("games").join(game_id.to_string())
    }

    fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::Other("database lock poisoned".into()))
    }

    pub fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn create_session(&self, token: &str, expires_at: &str) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO sessions (token, expires_at) VALUES (?1, ?2)",
            params![token, expires_at],
        )?;
        Ok(())
    }

    pub fn session_valid(&self, token: &str) -> AppResult<bool> {
        let conn = self.lock()?;
        let now = Utc::now().to_rfc3339();
        let found: Option<String> = conn
            .query_row(
                "SELECT token FROM sessions WHERE token = ?1 AND expires_at > ?2",
                params![token, now],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn delete_session(&self, token: &str) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    pub fn purge_expired_sessions(&self) -> AppResult<()> {
        let conn = self.lock()?;
        let now = Utc::now().to_rfc3339();
        conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now])?;
        Ok(())
    }

    fn map_game(row: &rusqlite::Row<'_>) -> rusqlite::Result<Game> {
        let tags_json: String = row.get(9)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let platforms_json: String = row.get(10)?;
        let platforms: Vec<String> = serde_json::from_str(&platforms_json).unwrap_or_default();
        let title_custom: i64 = row.get(3)?;
        Ok(Game {
            id: row.get(0)?,
            title: row.get(1)?,
            source_title: row.get(2)?,
            title_custom: title_custom != 0,
            f95_thread_id: row.get(4)?,
            f95_url: row.get(5)?,
            version: row.get(6)?,
            developer: row.get(7)?,
            tags,
            platforms,
            description: row.get(8)?,
            cover_image_path: row.get(11)?,
            rating: row.get(12)?,
            status: row.get(13)?,
            play_status: row.get(14)?,
            user_rating: row.get(15)?,
            user_notes: row.get(16)?,
            created_at: row.get(17)?,
            updated_at: row.get(18)?,
        })
    }

    const GAME_COLS: &'static str = "id, title, source_title, title_custom, f95_thread_id, f95_url,
        version, developer, description, tags, platforms, cover_image_path, rating, status, play_status,
        user_rating, user_notes, created_at, updated_at";

    pub fn get_game(&self, id: i64) -> AppResult<Game> {
        let conn = self.lock()?;
        conn.query_row(
            &format!("SELECT {} FROM games WHERE id = ?1", Self::GAME_COLS),
            params![id],
            Self::map_game,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("game {id}")),
            other => other.into(),
        })
    }

    pub fn get_game_by_thread(&self, thread_id: i64) -> AppResult<Option<Game>> {
        let conn = self.lock()?;
        conn.query_row(
            &format!(
                "SELECT {} FROM games WHERE f95_thread_id = ?1",
                Self::GAME_COLS
            ),
            params![thread_id],
            Self::map_game,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_games(&self, filter: &LibraryFilter) -> AppResult<Vec<Game>> {
        let conn = self.lock()?;
        let order = match filter.sort {
            LibrarySort::TitleAsc => "title COLLATE NOCASE ASC",
            LibrarySort::TitleDesc => "title COLLATE NOCASE DESC",
            LibrarySort::UpdatedDesc => "updated_at DESC",
            LibrarySort::RatingDesc => "rating IS NULL, rating DESC, title COLLATE NOCASE ASC",
            LibrarySort::UserRatingDesc => {
                "user_rating IS NULL, user_rating DESC, title COLLATE NOCASE ASC"
            }
        };

        let mut sql = format!("SELECT {} FROM games WHERE 1=1", Self::GAME_COLS);
        let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(search) = filter.search.as_ref().map(|s| s.trim().to_string()) {
            if !search.is_empty() {
                sql.push_str(" AND (title LIKE ? OR developer LIKE ? OR tags LIKE ?)");
                let like = format!("%{search}%");
                binds.push(Box::new(like.clone()));
                binds.push(Box::new(like.clone()));
                binds.push(Box::new(like));
            }
        }

        if let Some(status) = filter.play_status.as_ref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND play_status = ?");
            binds.push(Box::new(status.clone()));
        }
        if filter.unrated_only {
            sql.push_str(" AND user_rating IS NULL");
        } else if let Some(min) = filter.user_rating_min {
            sql.push_str(" AND user_rating IS NOT NULL AND user_rating >= ?");
            binds.push(Box::new(min));
        }

        sql.push_str(&format!(" ORDER BY {order}"));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            binds.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::map_game)?;
        let mut games = Vec::new();
        for row in rows {
            games.push(row?);
        }

        if !filter.tags.is_empty() {
            games.retain(|g| match filter.tag_mode {
                TagMode::And => filter.tags.iter().all(|t| g.tags.iter().any(|gt| gt == t)),
                TagMode::Or => filter.tags.iter().any(|t| g.tags.iter().any(|gt| gt == t)),
            });
        }

        if !filter.platforms.is_empty() {
            games.retain(|g| match filter.platform_mode {
                TagMode::And => filter
                    .platforms
                    .iter()
                    .all(|p| g.platforms.iter().any(|gp| gp.eq_ignore_ascii_case(p))),
                TagMode::Or => filter
                    .platforms
                    .iter()
                    .any(|p| g.platforms.iter().any(|gp| gp.eq_ignore_ascii_case(p))),
            });
        }

        Ok(games)
    }

    pub fn list_library_tags(&self) -> AppResult<Vec<LibraryTag>> {
        let games = self.list_games(&LibraryFilter::default())?;
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for game in games {
            for tag in game.tags {
                // SAM sometimes stores numeric IDs; only expose human-readable names.
                if tag.trim().is_empty() || tag.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                *counts.entry(tag).or_default() += 1;
            }
        }
        Ok(counts
            .into_iter()
            .map(|(tag, count)| LibraryTag { tag, count })
            .collect())
    }

    pub fn list_library_platforms(&self) -> AppResult<Vec<LibraryPlatform>> {
        let games = self.list_games(&LibraryFilter::default())?;
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for game in games {
            for platform in game.platforms {
                if platform.trim().is_empty() {
                    continue;
                }
                *counts.entry(platform).or_default() += 1;
            }
        }
        Ok(counts
            .into_iter()
            .map(|(platform, count)| LibraryPlatform { platform, count })
            .collect())
    }

    /// Platforms keyed by F95 thread id for hydrating catalog browse results.
    pub fn platforms_by_thread_ids(
        &self,
        thread_ids: &[i64],
    ) -> AppResult<std::collections::HashMap<i64, Vec<String>>> {
        if thread_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = self.lock()?;
        let mut map = std::collections::HashMap::new();
        for id in thread_ids {
            let platforms_json: Option<String> = conn
                .query_row(
                    "SELECT platforms FROM games WHERE f95_thread_id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(json) = platforms_json {
                let platforms: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
                if !platforms.is_empty() {
                    map.insert(*id, platforms);
                }
            }
        }
        Ok(map)
    }

    pub fn get_metadata_cache(&self, source: &str, external_id: &str) -> AppResult<Option<String>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT data FROM metadata_cache WHERE source = ?1 AND external_id = ?2",
            params![source, external_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn insert_game_from_f95(
        &self,
        title: &str,
        thread_id: i64,
        url: &str,
        version: Option<&str>,
        developer: Option<&str>,
        tags: &[String],
        platforms: &[String],
        description: Option<&str>,
        rating: Option<f64>,
        status: Option<&str>,
        cover_path: Option<&str>,
    ) -> AppResult<i64> {
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(tags)?;
        let platforms_json = serde_json::to_string(platforms)?;
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO games (
                title, source_title, title_custom, f95_thread_id, f95_url, version, developer,
                tags, platforms, description, cover_image_path, rating, status, play_status, created_at, updated_at
             ) VALUES (?1, ?1, 0, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'unplayed', ?12, ?12)",
            params![
                title,
                thread_id,
                url,
                version,
                developer,
                tags_json,
                platforms_json,
                description,
                cover_path,
                rating,
                status,
                now,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_game_metadata(
        &self,
        id: i64,
        title: &str,
        version: Option<&str>,
        developer: Option<&str>,
        tags: &[String],
        platforms: &[String],
        description: Option<&str>,
        rating: Option<f64>,
        status: Option<&str>,
        cover_path: Option<&str>,
        f95_url: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(tags)?;
        let platforms_json = serde_json::to_string(platforms)?;
        let game = self.get_game(id)?;
        let conn = self.lock()?;
        let changed = if game.title_custom {
            conn.execute(
                "UPDATE games SET
                    source_title = ?1, version = ?2, developer = ?3, tags = ?4, platforms = ?5,
                    description = ?6, rating = ?7, status = ?8,
                    cover_image_path = COALESCE(?9, cover_image_path),
                    f95_url = ?10, updated_at = ?11
                 WHERE id = ?12",
                params![
                    title,
                    version,
                    developer,
                    tags_json,
                    platforms_json,
                    description,
                    rating,
                    status,
                    cover_path,
                    f95_url,
                    now,
                    id
                ],
            )?
        } else {
            conn.execute(
                "UPDATE games SET
                    title = ?1, source_title = ?1, version = ?2, developer = ?3, tags = ?4,
                    platforms = ?5, description = ?6, rating = ?7, status = ?8,
                    cover_image_path = COALESCE(?9, cover_image_path),
                    f95_url = ?10, updated_at = ?11
                 WHERE id = ?12",
                params![
                    title,
                    version,
                    developer,
                    tags_json,
                    platforms_json,
                    description,
                    rating,
                    status,
                    cover_path,
                    f95_url,
                    now,
                    id
                ],
            )?
        };
        if changed == 0 {
            return Err(AppError::NotFound(format!("game {id}")));
        }
        Ok(())
    }

    pub fn set_cover_path(&self, id: i64, path: Option<&str>) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock()?;
        conn.execute(
            "UPDATE games SET cover_image_path = ?1, updated_at = ?2 WHERE id = ?3",
            params![path, now, id],
        )?;
        Ok(())
    }

    pub fn update_game_user_data(&self, id: i64, data: &UpdateGameUserData) -> AppResult<Game> {
        let game = self.get_game(id)?;
        let now = Utc::now().to_rfc3339();
        let play_status = data.play_status.clone().or(game.play_status);
        let user_rating = match data.user_rating {
            None => game.user_rating,
            Some(inner) => inner,
        };
        let user_notes = data.user_notes.clone().or(game.user_notes);
        let description = data.description.clone().or(game.description);

        let reset = data.reset_title.unwrap_or(false);
        let (title, title_custom) = if reset {
            let restored = game
                .source_title
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| game.title.clone());
            (restored, 0_i64)
        } else if let Some(ref custom) = data.title {
            let trimmed = custom.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest("Title cannot be empty".into()));
            }
            (trimmed.to_string(), 1_i64)
        } else {
            (game.title.clone(), if game.title_custom { 1 } else { 0 })
        };

        let conn = self.lock()?;
        conn.execute(
            "UPDATE games SET play_status = ?1, user_rating = ?2, user_notes = ?3,
                title = ?4, title_custom = ?5, description = ?6, updated_at = ?7 WHERE id = ?8",
            params![
                play_status,
                user_rating,
                user_notes,
                title,
                title_custom,
                description,
                now,
                id
            ],
        )?;
        drop(conn);
        self.get_game(id)
    }

    pub fn delete_game(&self, id: i64) -> AppResult<()> {
        let conn = self.lock()?;
        let changed = conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("game {id}")));
        }
        Ok(())
    }

    pub fn clear_game_media(&self, game_id: i64) -> AppResult<()> {
        let conn = self.lock()?;
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
        let now = Utc::now().to_rfc3339();
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO media (game_id, url, local_path, media_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![game_id, url, local_path, media_type, now],
        )?;
        Ok(())
    }

    pub fn list_game_media(&self, game_id: i64) -> AppResult<Vec<GameMediaRecord>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, media_type, url, local_path FROM media WHERE game_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![game_id], |row| {
            Ok(GameMediaRecord {
                id: row.get(0)?,
                media_type: row.get(1)?,
                source_url: row.get(2)?,
                local_path: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn upsert_metadata_cache(
        &self,
        source: &str,
        external_id: &str,
        title: Option<&str>,
        data: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO metadata_cache (source, external_id, title, data, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source, external_id) DO UPDATE SET
                title = excluded.title, data = excluded.data, fetched_at = excluded.fetched_at",
            params![source, external_id, title, data, now],
        )?;
        Ok(())
    }

    pub fn list_saves(&self, game_id: i64) -> AppResult<Vec<GameSave>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, game_id, path, filename, size, uploaded_at FROM game_saves
             WHERE game_id = ?1 ORDER BY uploaded_at DESC",
        )?;
        let rows = stmt.query_map(params![game_id], |row| {
            Ok(GameSave {
                id: row.get(0)?,
                game_id: row.get(1)?,
                path: row.get(2)?,
                filename: row.get(3)?,
                size: row.get(4)?,
                uploaded_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_save(
        &self,
        game_id: i64,
        path: &str,
        filename: &str,
        size: i64,
    ) -> AppResult<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO game_saves (game_id, path, filename, size, uploaded_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![game_id, path, filename, size, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_save(&self, id: i64) -> AppResult<GameSave> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, game_id, path, filename, size, uploaded_at FROM game_saves WHERE id = ?1",
            params![id],
            |row| {
                Ok(GameSave {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    path: row.get(2)?,
                    filename: row.get(3)?,
                    size: row.get(4)?,
                    uploaded_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("save {id}")),
            other => other.into(),
        })
    }

    pub fn delete_save(&self, id: i64) -> AppResult<GameSave> {
        let save = self.get_save(id)?;
        let conn = self.lock()?;
        conn.execute("DELETE FROM game_saves WHERE id = ?1", params![id])?;
        Ok(save)
    }

    pub fn list_patches(&self, game_id: i64) -> AppResult<Vec<GamePatch>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, game_id, path, filename, size, description, uploaded_at FROM game_patches
             WHERE game_id = ?1 ORDER BY uploaded_at DESC",
        )?;
        let rows = stmt.query_map(params![game_id], |row| {
            Ok(GamePatch {
                id: row.get(0)?,
                game_id: row.get(1)?,
                path: row.get(2)?,
                filename: row.get(3)?,
                size: row.get(4)?,
                description: row.get(5)?,
                uploaded_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_patch(
        &self,
        game_id: i64,
        path: &str,
        filename: &str,
        size: i64,
        description: Option<&str>,
    ) -> AppResult<i64> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO game_patches (game_id, path, filename, size, description, uploaded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![game_id, path, filename, size, description, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_patch(&self, id: i64) -> AppResult<GamePatch> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, game_id, path, filename, size, description, uploaded_at
             FROM game_patches WHERE id = ?1",
            params![id],
            |row| {
                Ok(GamePatch {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    path: row.get(2)?,
                    filename: row.get(3)?,
                    size: row.get(4)?,
                    description: row.get(5)?,
                    uploaded_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("patch {id}")),
            other => other.into(),
        })
    }

    pub fn delete_patch(&self, id: i64) -> AppResult<GamePatch> {
        let patch = self.get_patch(id)?;
        let conn = self.lock()?;
        conn.execute("DELETE FROM game_patches WHERE id = ?1", params![id])?;
        Ok(patch)
    }
}
