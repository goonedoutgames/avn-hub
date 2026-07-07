use crate::attachments::{
    ensure_parent_sync, flat_archive_dest, is_structured_archive_path, platform_archive_dest,
};
use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{GamePlatformArchive, ReorganizeResult};
use std::path::{Path, PathBuf};

fn normalize_path_string(path: &str) -> String {
    path.replace('\\', "/")
}

fn paths_equal(a: &str, b: &str) -> bool {
    normalize_path_string(a) == normalize_path_string(b)
}

/// Locate the on-disk file for a platform archive, tolerating stale DB paths.
fn resolve_archive_source(archive_root: &Path, archive: &GamePlatformArchive) -> PathBuf {
    let stored = Path::new(&archive.path);
    let mut candidates = vec![
        stored.to_path_buf(),
        archive_root.join(stored),
        flat_archive_dest(archive_root, &archive.filename),
    ];

    if is_structured_archive_path(archive.game_id, &archive.path) {
        candidates.push(platform_archive_dest(
            archive_root,
            archive.game_id,
            &archive.platform,
            &archive.filename,
        ));
    }

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    stored.to_path_buf()
}

pub fn reorganize_archive_file(
    db: &Database,
    archive_root: &str,
    archive: &GamePlatformArchive,
) -> AppResult<bool> {
    if archive.platform == "unknown" {
        return Ok(false);
    }

    let root = archive_root.trim();
    if root.is_empty() {
        return Err(AppError::BadRequest("archive path not configured".into()));
    }

    let archive_root_path = Path::new(root);
    let source = resolve_archive_source(archive_root_path, archive);
    if !source.exists() {
        return Err(AppError::NotFound(format!(
            "archive file not found (checked {} and flat path under archive folder)",
            archive.path
        )));
    }

    let dest = platform_archive_dest(
        archive_root_path,
        archive.game_id,
        &archive.platform,
        &archive.filename,
    );
    let dest_str = dest.to_string_lossy().to_string();

    if paths_equal(&archive.path, &dest_str) && source.exists() && paths_equal(
        source.to_string_lossy().as_ref(),
        &dest_str,
    ) {
        return Ok(false);
    }

    if dest.exists() && !paths_equal(source.to_string_lossy().as_ref(), &dest_str) {
        return Err(AppError::BadRequest(format!(
            "destination already exists: {}",
            dest.display()
        )));
    }

    ensure_parent_sync(&dest)?;

    if let Err(e) = std::fs::rename(&source, &dest) {
        std::fs::copy(&source, &dest).map_err(|copy_err| {
            AppError::Other(format!(
                "failed to move archive from {} to {} (rename: {e}, copy: {copy_err})",
                source.display(),
                dest.display()
            ))
        })?;
        std::fs::remove_file(&source)?;
    }

    let meta = std::fs::metadata(&dest)?;
    db.replace_platform_archive(
        archive.id,
        &dest_str,
        &archive.filename,
        meta.len() as i64,
    )?;
    Ok(true)
}

pub fn reorganize_all(
    db: &Database,
    archive_root: &str,
    include_unknown: bool,
) -> AppResult<ReorganizeResult> {
    let mut result = ReorganizeResult {
        moved: 0,
        skipped_unknown: 0,
        skipped_already_structured: 0,
        skipped_missing: 0,
        failed: 0,
        errors: Vec::new(),
    };

    let archives = db.list_migration_archives()?;
    for item in archives {
        let archive = db.get_platform_archive(item.id)?;
        if archive.platform == "unknown" {
            result.skipped_unknown += 1;
            if include_unknown {
                // reserved for a future "move unknown into unknown/" layout
            }
            continue;
        }

        let dest = platform_archive_dest(
            Path::new(archive_root),
            archive.game_id,
            &archive.platform,
            &archive.filename,
        );
        let dest_str = dest.to_string_lossy().to_string();
        let source = resolve_archive_source(Path::new(archive_root), &archive);

        if paths_equal(&archive.path, &dest_str)
            && source.exists()
            && paths_equal(source.to_string_lossy().as_ref(), &dest_str)
        {
            result.skipped_already_structured += 1;
            continue;
        }

        if !source.exists() {
            result.skipped_missing += 1;
            result.errors.push(format!(
                "{} ({}): file not found at {} or {}",
                item.game_title,
                item.filename,
                archive.path,
                flat_archive_dest(Path::new(archive_root), &archive.filename).display()
            ));
            continue;
        }

        match reorganize_archive_file(db, archive_root, &archive) {
            Ok(true) => result.moved += 1,
            Ok(false) => result.skipped_already_structured += 1,
            Err(e) => {
                result.failed += 1;
                result.errors.push(format!(
                    "{} ({}): {}",
                    item.game_title, item.filename, e
                ));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("avn-hub-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reorganize_moves_flat_file_when_db_path_is_stale() {
        let data_dir = temp_dir("data");
        let archive_root = temp_dir("archives");
        let db = Database::new(&data_dir).unwrap();

        let filename = "legacy-game-pc.zip";
        let flat_path = flat_archive_dest(&archive_root, filename);
        fs::write(&flat_path, b"zip-bytes").unwrap();

        let flat_str = flat_path.to_string_lossy().to_string();
        let (_is_new, game_id) = db.upsert_archive(&flat_str, filename, 128).unwrap();
        let archive_id = db.list_platform_archives(game_id).unwrap()[0].id;

        let stale_path = archive_root
            .join("wrong-location")
            .join(filename)
            .to_string_lossy()
            .to_string();
        db.replace_platform_archive(archive_id, &stale_path, filename, 128)
            .unwrap();

        let archive = db.get_platform_archive(archive_id).unwrap();
        let moved = reorganize_archive_file(
            &db,
            archive_root.to_str().unwrap(),
            &archive,
        )
        .unwrap();
        assert!(moved);

        let dest = platform_archive_dest(&archive_root, game_id, "windows_linux", filename);
        assert!(dest.exists());
        assert!(!flat_path.exists());

        let updated = db.get_platform_archive(archive_id).unwrap();
        assert_eq!(updated.path, dest.to_string_lossy());
    }
}
