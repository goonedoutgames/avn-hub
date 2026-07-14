//! Game file attachments: platform archives, saves, and patches.

use crate::error::AppResult;
use std::path::{Path, PathBuf};

pub const UPLOAD_KIND_ARCHIVE: &str = "archive";
pub const UPLOAD_KIND_SAVE: &str = "save";
pub const UPLOAD_KIND_PATCH: &str = "patch";

/// Extensions for scannable / uploadable game files (archives and Android packages).
pub const GAME_FILE_EXTENSIONS: &[&str] = &[
    "bz2", "rar", "zip", "7z", "apk", "xapk", "apks",
];

const PATCH_EXTENSIONS: &[&str] = &["zip", "rar", "7z", "bz2", "patch", "ppk", "exe", "apk"];

pub fn extension_ok(filename: &str, allowed: &[&str]) -> bool {
    let lower = filename.to_lowercase();
    allowed
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

pub fn is_game_file_filename(filename: &str) -> bool {
    extension_ok(filename, GAME_FILE_EXTENSIONS)
}

pub fn sanitize_game_file_filename(filename: &str) -> Option<String> {
    let base = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())?;
    if base.is_empty() || base.contains("..") || base.contains('/') || base.contains('\\') {
        return None;
    }
    if is_game_file_filename(base) {
        Some(base.to_string())
    } else {
        None
    }
}

pub fn normalize_upload_kind(value: &str) -> Option<&'static str> {
    match value.trim().to_lowercase().as_str() {
        "archive" | "archives" => Some(UPLOAD_KIND_ARCHIVE),
        "save" | "saves" => Some(UPLOAD_KIND_SAVE),
        "patch" | "patches" => Some(UPLOAD_KIND_PATCH),
        _ => None,
    }
}

pub fn sanitize_filename(filename: &str, kind: &str) -> Option<String> {
    let base = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())?;
    if base.is_empty() || base.contains("..") || base.contains('/') || base.contains('\\') {
        return None;
    }
    match kind {
        UPLOAD_KIND_ARCHIVE if is_game_file_filename(base) => Some(base.to_string()),
        UPLOAD_KIND_PATCH if extension_ok(base, PATCH_EXTENSIONS) => Some(base.to_string()),
        UPLOAD_KIND_SAVE => Some(base.to_string()),
        _ => None,
    }
}

pub fn platform_archive_dest(
    archive_root: &Path,
    game_id: i64,
    platform: &str,
    filename: &str,
) -> PathBuf {
    archive_root
        .join("games")
        .join(game_id.to_string())
        .join("platforms")
        .join(platform)
        .join(filename)
}

pub fn patch_dest(archive_root: &Path, game_id: i64, filename: &str) -> PathBuf {
    archive_root
        .join("games")
        .join(game_id.to_string())
        .join("patches")
        .join(filename)
}

pub fn save_dest(data_dir: &Path, game_id: i64, filename: &str) -> PathBuf {
    data_dir
        .join("games")
        .join(game_id.to_string())
        .join("saves")
        .join(filename)
}

pub fn flat_archive_dest(archive_root: &Path, filename: &str) -> PathBuf {
    archive_root.join(filename)
}

pub async fn ensure_parent(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    Ok(())
}

pub fn ensure_parent_sync(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Staging directory for in-progress TUS uploads.
/// Archive/patch uploads stage under the archive root so finalize can rename
/// within the same mount (Docker often separates `/data` and `/archives`).
pub fn tus_staging_dir(data_dir: &Path, archive_root: &Path, upload_kind: &str) -> PathBuf {
    if upload_kind == UPLOAD_KIND_SAVE {
        data_dir.join("uploads")
    } else {
        archive_root.join(".avn-hub-uploads")
    }
}

/// Rename, falling back to copy+delete when volumes differ (common with Docker).
pub async fn move_file(from: &Path, to: &Path) -> AppResult<()> {
    if tokio::fs::rename(from, to).await.is_ok() {
        return Ok(());
    }
    tokio::fs::copy(from, to).await.map_err(|copy_err| {
        crate::error::AppError::Other(format!(
            "failed to move {} to {} (copy after rename failed: {copy_err})",
            from.display(),
            to.display()
        ))
    })?;
    tokio::fs::remove_file(from).await?;
    Ok(())
}

pub fn is_path_under_root(file_path: &str, root: &str) -> bool {
    let root = root.trim().trim_end_matches('/');
    if root.is_empty() {
        return false;
    }
    let file = file_path.trim();
    file == root
        || file.starts_with(&format!("{root}/"))
        || file.starts_with(&format!("{root}\\"))
}

pub fn is_structured_archive_path(game_id: i64, path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.contains(&format!("/games/{game_id}/platforms/"))
}
