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

        Ok(Settings {
            archive_path,
            data_dir: self.data_dir.display().to_string(),
            f95_username,
            f95_password_set: f95_password.is_some_and(|p| !p.is_empty()),
            f95_cookies,
            f95_authenticated,
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
        drop(conn);
        self.get_settings()
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
    ) -> AppResult<Vec<Game>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, archive_path, archive_filename, archive_size,
                    f95_thread_id, f95_url, version, developer, tags, description,
                    cover_image_path, rating, status, matched, created_at, updated_at
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
                    cover_image_path, rating, status, matched, created_at, updated_at
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
            matched: row.get::<_, i64>(14)? != 0,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    }

    pub fn list_archives(&self) -> AppResult<Vec<ArchiveEntry>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT archive_path, archive_filename, archive_size, matched, id
             FROM games ORDER BY archive_filename COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ArchiveEntry {
                path: row.get(0)?,
                filename: row.get(1)?,
                size: row.get(2)?,
                matched: row.get::<_, i64>(3)? != 0,
                game_id: Some(row.get(4)?),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn upsert_archive(
        &self,
        path: &str,
        filename: &str,
        size: i64,
    ) -> AppResult<(bool, i64)> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();
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
                return Ok((false, id));
            }
            return Ok((false, id));
        }

        let title = filename
            .rsplit_once('.')
            .map(|(name, _)| name.to_string())
            .unwrap_or_else(|| filename.to_string());

        conn.execute(
            "INSERT INTO games (title, archive_path, archive_filename, archive_size,
                                matched, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![title, path, filename, size, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok((true, id))
    }

    pub fn apply_metadata_match(
        &self,
        archive_path: &str,
        result: &F95SearchResult,
        cover_path: Option<String>,
        description: Option<String>,
    ) -> AppResult<Game> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();
        let tags_json = serde_json::to_string(&result.tags)?;

        let updated = conn.execute(
            "UPDATE games SET
                title = ?1, f95_thread_id = ?2, f95_url = ?3, version = ?4,
                developer = ?5, tags = ?6, cover_image_path = ?7, rating = ?8,
                description = ?9, matched = 1, updated_at = ?10
             WHERE archive_path = ?11",
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
                archive_path,
            ],
        )?;

        if updated == 0 {
            return Err(AppError::NotFound(format!(
                "archive not found: {archive_path}"
            )));
        }

        let id: i64 = conn.query_row(
            "SELECT id FROM games WHERE archive_path = ?1",
            params![archive_path],
            |row| row.get(0),
        )?;

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
        self.get_game(id)
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
