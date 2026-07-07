use crate::error::{AppError, AppResult};
use crate::models::{ArchiveEntry, GamePatch, GamePlatformArchive, GameSave};
use crate::platform::{detect_platform_from_filename, normalize_platform};
use rusqlite::{params, OptionalExtension};
use super::Database;

impl Database {
    pub fn migrate_attachments(&self) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;

        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS game_platform_archives (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                platform TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                filename TEXT NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                is_default INTEGER NOT NULL DEFAULT 0,
                uploaded_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE (game_id, platform)
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_game_default_platform_archive
             ON game_platform_archives (game_id) WHERE is_default = 1",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS game_saves (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                path TEXT NOT NULL UNIQUE,
                filename TEXT NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                uploaded_at TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS game_patches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                path TEXT NOT NULL UNIQUE,
                filename TEXT NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                description TEXT,
                uploaded_at TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tus_uploads ADD COLUMN upload_kind TEXT NOT NULL DEFAULT 'archive'",
            [],
        );
        let _ = conn.execute("ALTER TABLE tus_uploads ADD COLUMN platform TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE tus_uploads ADD COLUMN replace_archive_id INTEGER",
            [],
        );

        self.backfill_platform_archives(&conn)?;
        drop(conn);
        self.migrate_platform_check_constraint()?;

        Ok(())
    }

    /// SQLite cannot alter CHECK constraints; recreate the table when the platform list changes.
    fn migrate_platform_check_constraint(&self) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let ddl: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'game_platform_archives'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        let Some(ddl) = ddl else {
            return Ok(());
        };

        let has_platform_check = ddl.contains("CHECK (platform IN");
        let includes_windows_linux = ddl.contains("windows_linux");

        if !has_platform_check || includes_windows_linux {
            return Ok(());
        }

        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             BEGIN;
             CREATE TABLE game_platform_archives_new (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                 platform TEXT NOT NULL,
                 path TEXT NOT NULL UNIQUE,
                 filename TEXT NOT NULL,
                 size INTEGER NOT NULL DEFAULT 0,
                 is_default INTEGER NOT NULL DEFAULT 0,
                 uploaded_at TEXT,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 UNIQUE (game_id, platform)
             );
             INSERT INTO game_platform_archives_new
                 (id, game_id, platform, path, filename, size, is_default, uploaded_at, created_at, updated_at)
             SELECT id, game_id, platform, path, filename, size, is_default, uploaded_at, created_at, updated_at
             FROM game_platform_archives;
             DROP TABLE game_platform_archives;
             ALTER TABLE game_platform_archives_new RENAME TO game_platform_archives;
             CREATE UNIQUE INDEX IF NOT EXISTS idx_game_default_platform_archive
                 ON game_platform_archives (game_id) WHERE is_default = 1;
             CREATE INDEX IF NOT EXISTS idx_platform_archives_game ON game_platform_archives(game_id);
             COMMIT;
             PRAGMA foreign_keys = ON;",
        )?;

        Ok(())
    }

    fn backfill_platform_archives(&self, conn: &rusqlite::Connection) -> AppResult<()> {
        let mut stmt = conn.prepare(
            "SELECT g.id, g.archive_path, g.archive_filename, g.archive_size, g.created_at, g.updated_at
             FROM games g
             WHERE g.archive_path IS NOT NULL AND g.archive_path != ''
             AND NOT EXISTS (
                 SELECT 1 FROM game_platform_archives a WHERE a.game_id = g.id
             )",
        )?;
        let rows: Vec<(i64, String, String, i64, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        for (game_id, path, filename, size, created_at, updated_at) in rows {
            let platform = detect_platform_from_filename(&filename).to_string();
            let _ = conn.execute(
                "INSERT OR IGNORE INTO game_platform_archives
                    (game_id, platform, path, filename, size, is_default, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
                params![game_id, platform, path, filename, size, created_at, updated_at],
            );
        }
        Ok(())
    }

    pub fn list_migration_archives(&self) -> AppResult<Vec<crate::models::MigrationArchiveItem>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.game_id, g.title, a.filename, a.platform, a.path, a.is_default
             FROM game_platform_archives a
             JOIN games g ON g.id = a.game_id
             WHERE g.matched = 1
             ORDER BY g.title COLLATE NOCASE, a.filename COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let game_id: i64 = row.get(1)?;
            let path: String = row.get(5)?;
            let platform: String = row.get(4)?;
            let is_legacy = !crate::attachments::is_structured_archive_path(game_id, &path);
            let needs_platform = platform == "unknown";
            Ok(crate::models::MigrationArchiveItem {
                id,
                game_id,
                game_title: row.get(2)?,
                filename: row.get(3)?,
                platform,
                path,
                is_default: row.get::<_, i64>(6)? != 0,
                is_legacy_path: is_legacy,
                needs_platform,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn update_archive_platform(
        &self,
        archive_id: i64,
        platform: &str,
    ) -> AppResult<(GamePlatformArchive, Option<i64>)> {
        let platform = normalize_platform(platform).unwrap_or_else(|| "unknown".into());
        let archive = self.get_platform_archive(archive_id)?;

        if platform == archive.platform {
            return Ok((archive, None));
        }

        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();

        let swapped_id: Option<i64> = if platform != "unknown" {
            conn
                .query_row(
                    "SELECT id FROM game_platform_archives
                     WHERE game_id = ?1 AND platform = ?2 AND id != ?3",
                    params![archive.game_id, platform, archive_id],
                    |row| row.get(0),
                )
                .optional()?
        } else {
            None
        };

        if let Some(other_id) = swapped_id {
            let old_platform = archive.platform.clone();
            conn.execute(
                "UPDATE game_platform_archives SET platform = ?1, updated_at = ?2 WHERE id = ?3",
                params![old_platform, now, other_id],
            )?;
            conn.execute(
                "UPDATE game_platform_archives SET platform = ?1, updated_at = ?2 WHERE id = ?3",
                params![platform, now, archive_id],
            )?;
        } else {
            conn.execute(
                "UPDATE game_platform_archives SET platform = ?1, updated_at = ?2 WHERE id = ?3",
                params![platform, now, archive_id],
            )?;
        }

        drop(conn);
        Ok((self.get_platform_archive(archive_id)?, swapped_id))
    }

    pub fn sync_game_default_archive(&self, game_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let default: Option<(String, String, i64)> = conn
            .query_row(
                "SELECT path, filename, size FROM game_platform_archives
                 WHERE game_id = ?1 AND is_default = 1 LIMIT 1",
                params![game_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        if let Some((path, filename, size)) = default {
            conn.execute(
                "UPDATE games SET archive_path = ?1, archive_filename = ?2, archive_size = ?3,
                 updated_at = ?4 WHERE id = ?5",
                params![path, filename, size, Self::now(), game_id],
            )?;
        }
        Ok(())
    }

    pub fn list_platform_archives(&self, game_id: i64) -> AppResult<Vec<GamePlatformArchive>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, game_id, platform, path, filename, size, is_default, uploaded_at,
                    created_at, updated_at
             FROM game_platform_archives WHERE game_id = ?1 ORDER BY is_default DESC, platform",
        )?;
        let rows = stmt.query_map(params![game_id], Self::row_to_platform_archive)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn get_platform_archive(&self, id: i64) -> AppResult<GamePlatformArchive> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.query_row(
            "SELECT id, game_id, platform, path, filename, size, is_default, uploaded_at,
                    created_at, updated_at
             FROM game_platform_archives WHERE id = ?1",
            params![id],
            Self::row_to_platform_archive,
        )
        .map_err(|_| AppError::NotFound(format!("platform archive {id} not found")))
    }

    pub fn list_game_saves(&self, game_id: i64) -> AppResult<Vec<GameSave>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, game_id, path, filename, size, uploaded_at
             FROM game_saves WHERE game_id = ?1 ORDER BY uploaded_at DESC",
        )?;
        let rows = stmt.query_map(params![game_id], Self::row_to_game_save)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_game_patches(&self, game_id: i64) -> AppResult<Vec<GamePatch>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, game_id, path, filename, size, description, uploaded_at
             FROM game_patches WHERE game_id = ?1 ORDER BY uploaded_at DESC",
        )?;
        let rows = stmt.query_map(params![game_id], Self::row_to_game_patch)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn insert_platform_archive(
        &self,
        game_id: i64,
        platform: &str,
        path: &str,
        filename: &str,
        size: i64,
        is_default: bool,
        uploaded_at: Option<&str>,
    ) -> AppResult<GamePlatformArchive> {
        let platform = normalize_platform(platform).unwrap_or_else(|| "unknown".into());
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();

        if is_default {
            conn.execute(
                "UPDATE game_platform_archives SET is_default = 0 WHERE game_id = ?1",
                params![game_id],
            )?;
        }

        let has_default: i64 = conn.query_row(
            "SELECT COUNT(*) FROM game_platform_archives WHERE game_id = ?1 AND is_default = 1",
            params![game_id],
            |row| row.get(0),
        )?;
        let make_default = is_default || has_default == 0;

        conn.execute(
            "INSERT INTO game_platform_archives
                (game_id, platform, path, filename, size, is_default, uploaded_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(path) DO UPDATE SET
                filename = excluded.filename,
                size = excluded.size,
                platform = excluded.platform,
                is_default = excluded.is_default,
                uploaded_at = excluded.uploaded_at,
                updated_at = excluded.updated_at",
            params![
                game_id,
                platform,
                path,
                filename,
                size,
                if make_default { 1 } else { 0 },
                uploaded_at,
                now,
            ],
        )?;

        let id: i64 = conn.query_row(
            "SELECT id FROM game_platform_archives WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        drop(conn);
        self.sync_game_default_archive(game_id)?;
        self.get_platform_archive(id)
    }

    pub fn replace_platform_archive(
        &self,
        archive_id: i64,
        path: &str,
        filename: &str,
        size: i64,
    ) -> AppResult<GamePlatformArchive> {
        let existing = self.get_platform_archive(archive_id)?;
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE game_platform_archives SET path = ?1, filename = ?2, size = ?3, updated_at = ?4
             WHERE id = ?5",
            params![path, filename, size, Self::now(), archive_id],
        )?;
        drop(conn);
        self.sync_game_default_archive(existing.game_id)?;
        self.get_platform_archive(archive_id)
    }

    pub fn set_default_platform_archive(&self, archive_id: i64) -> AppResult<GamePlatformArchive> {
        let archive = self.get_platform_archive(archive_id)?;
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE game_platform_archives SET is_default = 0 WHERE game_id = ?1",
            params![archive.game_id],
        )?;
        conn.execute(
            "UPDATE game_platform_archives SET is_default = 1, updated_at = ?1 WHERE id = ?2",
            params![Self::now(), archive_id],
        )?;
        drop(conn);
        self.sync_game_default_archive(archive.game_id)?;
        self.get_platform_archive(archive_id)
    }

    pub fn delete_platform_archive(&self, archive_id: i64) -> AppResult<i64> {
        let archive = self.get_platform_archive(archive_id)?;
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "DELETE FROM game_platform_archives WHERE id = ?1",
            params![archive_id],
        )?;
        drop(conn);

        if archive.is_default {
            let remaining = self.list_platform_archives(archive.game_id)?;
            if let Some(first) = remaining.first() {
                let _ = self.set_default_platform_archive(first.id);
            } else {
                let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
                conn.execute(
                    "UPDATE games SET archive_path = '', archive_filename = '', archive_size = 0,
                     updated_at = ?1 WHERE id = ?2",
                    params![Self::now(), archive.game_id],
                )?;
            }
        } else {
            self.sync_game_default_archive(archive.game_id)?;
        }
        Ok(archive.game_id)
    }

    pub fn insert_game_save(
        &self,
        game_id: i64,
        path: &str,
        filename: &str,
        size: i64,
    ) -> AppResult<GameSave> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();
        conn.execute(
            "INSERT INTO game_saves (game_id, path, filename, size, uploaded_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET filename = excluded.filename, size = excluded.size,
             uploaded_at = excluded.uploaded_at",
            params![game_id, path, filename, size, now],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM game_saves WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        drop(conn);
        self.get_game_save(id)
    }

    pub fn get_game_save(&self, id: i64) -> AppResult<GameSave> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.query_row(
            "SELECT id, game_id, path, filename, size, uploaded_at FROM game_saves WHERE id = ?1",
            params![id],
            Self::row_to_game_save,
        )
        .map_err(|_| AppError::NotFound(format!("save {id} not found")))
    }

    pub fn delete_game_save(&self, id: i64) -> AppResult<GameSave> {
        let save = self.get_game_save(id)?;
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute("DELETE FROM game_saves WHERE id = ?1", params![id])?;
        Ok(save)
    }

    pub fn insert_game_patch(
        &self,
        game_id: i64,
        path: &str,
        filename: &str,
        size: i64,
        description: Option<&str>,
    ) -> AppResult<GamePatch> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let now = Self::now();
        conn.execute(
            "INSERT INTO game_patches (game_id, path, filename, size, description, uploaded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET filename = excluded.filename, size = excluded.size,
             description = excluded.description, uploaded_at = excluded.uploaded_at",
            params![game_id, path, filename, size, description, now],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM game_patches WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        drop(conn);
        self.get_game_patch(id)
    }

    pub fn get_game_patch(&self, id: i64) -> AppResult<GamePatch> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.query_row(
            "SELECT id, game_id, path, filename, size, description, uploaded_at
             FROM game_patches WHERE id = ?1",
            params![id],
            Self::row_to_game_patch,
        )
        .map_err(|_| AppError::NotFound(format!("patch {id} not found")))
    }

    pub fn delete_game_patch(&self, id: i64) -> AppResult<GamePatch> {
        let patch = self.get_game_patch(id)?;
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute("DELETE FROM game_patches WHERE id = ?1", params![id])?;
        Ok(patch)
    }

    pub fn set_platform_for_path(&self, path: &str, platform: &str) -> AppResult<()> {
        let platform = normalize_platform(platform).unwrap_or_else(|| "unknown".into());
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute(
            "UPDATE game_platform_archives SET platform = ?1, updated_at = ?2 WHERE path = ?3",
            params![platform, Self::now(), path],
        )?;
        Ok(())
    }

    pub fn list_archives_from_platform_table(&self) -> AppResult<Vec<ArchiveEntry>> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.path, a.filename, a.size, a.platform, g.matched, g.id
             FROM game_platform_archives a
             JOIN games g ON g.id = a.game_id
             ORDER BY a.filename COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ArchiveEntry {
                id: row.get(0)?,
                path: row.get(1)?,
                filename: row.get(2)?,
                size: row.get(3)?,
                platform: row.get(4)?,
                matched: row.get::<_, i64>(5)? != 0,
                game_id: Some(row.get(6)?),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn row_to_platform_archive(row: &rusqlite::Row<'_>) -> rusqlite::Result<GamePlatformArchive> {
        Ok(GamePlatformArchive {
            id: row.get(0)?,
            game_id: row.get(1)?,
            platform: row.get(2)?,
            path: row.get(3)?,
            filename: row.get(4)?,
            size: row.get(5)?,
            is_default: row.get::<_, i64>(6)? != 0,
            uploaded_at: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }

    fn row_to_game_save(row: &rusqlite::Row<'_>) -> rusqlite::Result<GameSave> {
        Ok(GameSave {
            id: row.get(0)?,
            game_id: row.get(1)?,
            path: row.get(2)?,
            filename: row.get(3)?,
            size: row.get(4)?,
            uploaded_at: row.get(5)?,
        })
    }

    fn row_to_game_patch(row: &rusqlite::Row<'_>) -> rusqlite::Result<GamePatch> {
        Ok(GamePatch {
            id: row.get(0)?,
            game_id: row.get(1)?,
            path: row.get(2)?,
            filename: row.get(3)?,
            size: row.get(4)?,
            description: row.get(5)?,
            uploaded_at: row.get(6)?,
        })
    }
}
