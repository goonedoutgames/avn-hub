use crate::api::auth::require_session;
use crate::attachments::{
    ensure_parent, flat_archive_dest, move_file, normalize_upload_kind, patch_dest,
    platform_archive_dest, save_dest, sanitize_filename, tus_staging_dir, UPLOAD_KIND_ARCHIVE,
    UPLOAD_KIND_PATCH, UPLOAD_KIND_SAVE,
};
use crate::error::{AppError, AppResult};
use crate::platform::{detect_platform_from_filename, normalize_platform};
use crate::scanner::{is_path_under_archive_root, sanitize_archive_filename};
use crate::state::SharedState;
use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::Engine;
use std::path::{Path as FsPath, PathBuf};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

const TUS_VERSION: &str = "1.0.0";

struct UploadMetadata {
    filename: String,
    replace_game_id: Option<i64>,
    upload_kind: &'static str,
    platform: Option<String>,
    replace_archive_id: Option<i64>,
}

fn tus_headers() -> [(header::HeaderName, &'static str); 2] {
    [
        (header::HeaderName::from_static("tus-resumable"), TUS_VERSION),
        (
            header::HeaderName::from_static("access-control-expose-headers"),
            "Upload-Offset, Upload-Length, Location, Tus-Resumable",
        ),
    ]
}

fn parse_upload_metadata(raw: &str) -> Option<UploadMetadata> {
    let mut filename = None;
    let mut replace_game_id = None;
    let mut upload_kind = UPLOAD_KIND_ARCHIVE;
    let mut platform = None;
    let mut replace_archive_id = None;

    for part in raw.split(',') {
        let part = part.trim();
        let Some((key, value)) = part.split_once(' ') else {
            continue;
        };
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(value.trim())
            .ok()?;
        let text = String::from_utf8(decoded).ok()?;
        match key {
            "filename" => filename = Some(text),
            "game_id" => replace_game_id = text.parse().ok(),
            "kind" => {
                upload_kind = normalize_upload_kind(&text)?;
            }
            "platform" => {
                platform = normalize_platform(&text).or_else(|| {
                    let detected = detect_platform_from_filename(&text);
                    if detected == "unknown" {
                        None
                    } else {
                        Some(detected.to_string())
                    }
                });
            }
            "replace_archive_id" => replace_archive_id = text.parse().ok(),
            _ => {}
        }
    }

    Some(UploadMetadata {
        filename: filename?,
        replace_game_id,
        upload_kind,
        platform,
        replace_archive_id,
    })
}

fn validate_filename(parsed: &UploadMetadata) -> AppResult<String> {
    if parsed.upload_kind == UPLOAD_KIND_ARCHIVE {
        sanitize_archive_filename(&parsed.filename)
            .ok_or_else(|| AppError::BadRequest("invalid archive filename".into()))
    } else {
        sanitize_filename(&parsed.filename, parsed.upload_kind)
            .ok_or_else(|| AppError::BadRequest("invalid filename for upload kind".into()))
    }
}

fn resolve_staging_dir(state: &SharedState, upload_kind: &str) -> AppResult<PathBuf> {
    let settings = state.get_settings()?;
    let archive_root = settings.archive_path.trim();
    if archive_root.is_empty() {
        return Err(AppError::BadRequest(
            "archive path not configured. Set it in Settings to /archives (Docker) first.".into(),
        ));
    }
    Ok(tus_staging_dir(
        state.db.data_dir(),
        FsPath::new(archive_root),
        upload_kind,
    ))
}

fn resolve_partial_path(state: &SharedState, id: &str, upload_kind: &str) -> AppResult<PathBuf> {
    let staging_dir = resolve_staging_dir(state, upload_kind)?;
    let partial_path = staging_dir.join(format!("{id}.partial"));
    if partial_path.exists() {
        return Ok(partial_path);
    }
    // Pre-fix uploads staged under /data/uploads — still finalize from there.
    let legacy = state.db.uploads_dir().join(format!("{id}.partial"));
    if legacy.exists() {
        return Ok(legacy);
    }
    Ok(partial_path)
}

pub async fn tus_options() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [
            (header::HeaderName::from_static("tus-resumable"), TUS_VERSION),
            (
                header::HeaderName::from_static("tus-extension"),
                "creation,expiration",
            ),
            (
                header::HeaderName::from_static("tus-max-size"),
                "107374182400",
            ),
        ],
    )
}

pub async fn tus_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    require_session(&state, &headers).await?;

    if headers
        .get("tus-resumable")
        .and_then(|v| v.to_str().ok())
        != Some(TUS_VERSION)
    {
        return Err(AppError::BadRequest("missing Tus-Resumable header".into()));
    }

    let length: i64 = headers
        .get("upload-length")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("missing Upload-Length".into()))?
        .parse()
        .map_err(|_| AppError::BadRequest("invalid Upload-Length".into()))?;

    if length <= 0 {
        return Err(AppError::BadRequest("Upload-Length must be positive".into()));
    }

    let metadata = headers
        .get("upload-metadata")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("missing Upload-Metadata".into()))?;
    let parsed = parse_upload_metadata(metadata)
        .ok_or_else(|| AppError::BadRequest("missing filename in Upload-Metadata".into()))?;
    let filename = validate_filename(&parsed)?;

    let settings = state.get_settings()?;
    if settings.archive_path.trim().is_empty() {
        return Err(AppError::BadRequest(
            "archive path not configured. Set it in Settings to /archives (Docker) first.".into(),
        ));
    }

    let game_id = parsed.replace_game_id;
    if matches!(parsed.upload_kind, UPLOAD_KIND_SAVE | UPLOAD_KIND_PATCH) && game_id.is_none() {
        return Err(AppError::BadRequest(
            "game_id required for save and patch uploads".into(),
        ));
    }

    if let Some(game_id) = game_id {
        state.db.get_game(game_id).map_err(|_| {
            AppError::BadRequest(format!("game {game_id} not found for upload"))
        })?;
    }

    if let Some(archive_id) = parsed.replace_archive_id {
        let archive = state.db.get_platform_archive(archive_id)?;
        if game_id.is_some_and(|gid| gid != archive.game_id) {
            return Err(AppError::BadRequest(
                "replace_archive_id does not belong to game_id".into(),
            ));
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let staging_dir = resolve_staging_dir(&state, parsed.upload_kind)?;
    tokio::fs::create_dir_all(&staging_dir).await.map_err(|e| {
        AppError::Other(format!(
            "cannot create upload staging dir {}: {e} (is /archives mounted read-write?)",
            staging_dir.display()
        ))
    })?;
    let partial_path = staging_dir.join(format!("{id}.partial"));
    tokio::fs::File::create(&partial_path).await.map_err(|e| {
        AppError::Other(format!(
            "cannot create upload file {}: {e}",
            partial_path.display()
        ))
    })?;

    state.db.create_tus_upload(
        &id,
        &filename,
        length,
        game_id,
        parsed.upload_kind,
        parsed.platform.as_deref(),
        parsed.replace_archive_id,
    )?;

    let location = format!("/api/tus/{id}");
    let mut response = Response::builder()
        .status(StatusCode::CREATED)
        .header(header::LOCATION, location)
        .header("Upload-Offset", "0")
        .header("Tus-Resumable", TUS_VERSION);
    for (name, value) in tus_headers() {
        response = response.header(name, value);
    }
    Ok(response.body(Body::empty()).unwrap())
}

pub async fn tus_head(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    require_session(&state, &headers).await?;

    let (_, size, offset, _, _, _, _) = state
        .db
        .get_tus_upload(&id)?
        .ok_or_else(|| AppError::NotFound("upload not found".into()))?;

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Upload-Offset", offset.to_string())
        .header("Upload-Length", size.to_string())
        .header("Tus-Resumable", TUS_VERSION);
    for (name, value) in tus_headers() {
        response = response.header(name, value);
    }
    Ok(response.body(Body::empty()).unwrap())
}

pub async fn tus_patch(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    require_session(&state, &headers).await?;

    if headers
        .get("tus-resumable")
        .and_then(|v| v.to_str().ok())
        != Some(TUS_VERSION)
    {
        return Err(AppError::BadRequest("missing Tus-Resumable header".into()));
    }

    let offset_header: i64 = headers
        .get("upload-offset")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("missing Upload-Offset".into()))?
        .parse()
        .map_err(|_| AppError::BadRequest("invalid Upload-Offset".into()))?;

    let (filename, size, stored_offset, game_id, upload_kind, platform, replace_archive_id) = state
        .db
        .get_tus_upload(&id)?
        .ok_or_else(|| AppError::NotFound("upload not found".into()))?;

    let staging_dir = resolve_staging_dir(&state, &upload_kind)?;
    let partial_path = resolve_partial_path(&state, &id, &upload_kind)?;

    // Recover uploads that wrote all bytes but failed finalize (e.g. cross-volume rename).
    if stored_offset == size {
        if !partial_path.exists() {
            // Already finalized / cleaned up on a racing retry.
            return tus_offset_response(size);
        }
        finalize_upload(
            &state,
            &id,
            &filename,
            &partial_path,
            game_id,
            &upload_kind,
            platform.as_deref(),
            replace_archive_id,
        )
        .await?;
        return tus_offset_response(size);
    }

    if offset_header != stored_offset {
        return Err(AppError::BadRequest(format!(
            "offset mismatch: expected {stored_offset}, got {offset_header}"
        )));
    }

    if body.is_empty() {
        return Err(AppError::BadRequest("empty upload chunk".into()));
    }

    // Prefer writing into the resolved path; create staging dir if this is a new file.
    if !partial_path.exists() {
        tokio::fs::create_dir_all(&staging_dir).await?;
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&partial_path)
        .await?;
    file.seek(std::io::SeekFrom::Start(stored_offset as u64)).await?;
    file.write_all(&body).await?;
    file.flush().await?;

    let new_offset = stored_offset + body.len() as i64;
    if new_offset > size {
        return Err(AppError::BadRequest("upload exceeds declared length".into()));
    }

    // Persist progress only after a successful write. Finalize before marking complete
    // so a failed move does not leave the upload stuck at 100%.
    if new_offset == size {
        finalize_upload(
            &state,
            &id,
            &filename,
            &partial_path,
            game_id,
            &upload_kind,
            platform.as_deref(),
            replace_archive_id,
        )
        .await?;
    } else {
        state.db.update_tus_offset(&id, new_offset)?;
    }

    tus_offset_response(new_offset)
}

fn tus_offset_response(offset: i64) -> AppResult<Response> {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Upload-Offset", offset.to_string())
        .header("Tus-Resumable", TUS_VERSION);
    for (name, value) in tus_headers() {
        response = response.header(name, value);
    }
    Ok(response.body(Body::empty()).unwrap())
}

async fn finalize_upload(
    state: &SharedState,
    id: &str,
    filename: &str,
    partial_path: &PathBuf,
    game_id: Option<i64>,
    upload_kind: &str,
    platform: Option<&str>,
    replace_archive_id: Option<i64>,
) -> AppResult<()> {
    let settings = state.get_settings()?;
    let archive_root = FsPath::new(&settings.archive_path);
    let data_dir = state.db.data_dir();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let (dest, dest_str) = match upload_kind {
        UPLOAD_KIND_SAVE => {
            let gid = game_id.ok_or_else(|| {
                AppError::BadRequest("game_id required for save upload".into())
            })?;
            let dest = save_dest(data_dir, gid, filename);
            let dest_str = dest.to_string_lossy().to_string();
            (dest, dest_str)
        }
        UPLOAD_KIND_PATCH => {
            let gid = game_id.ok_or_else(|| {
                AppError::BadRequest("game_id required for patch upload".into())
            })?;
            let dest = patch_dest(archive_root, gid, filename);
            let dest_str = dest.to_string_lossy().to_string();
            (dest, dest_str)
        }
        UPLOAD_KIND_ARCHIVE => {
            if let Some(gid) = game_id {
                let platform = platform
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| detect_platform_from_filename(filename).to_string());
                let dest = platform_archive_dest(archive_root, gid, &platform, filename);
                let dest_str = dest.to_string_lossy().to_string();
                (dest, dest_str)
            } else {
                let dest = flat_archive_dest(archive_root, filename);
                let dest_str = dest.to_string_lossy().to_string();
                (dest, dest_str)
            }
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "unsupported upload kind: {upload_kind}"
            )));
        }
    };

    ensure_parent(&dest).await?;

    if let Some(archive_id) = replace_archive_id {
        let existing = state.db.get_platform_archive(archive_id)?;
        if !is_path_under_archive_root(&existing.path, &settings.archive_path) {
            return Err(AppError::BadRequest(
                "existing archive is outside the configured folder".into(),
            ));
        }
        if dest.exists() && dest_str != existing.path {
            return Err(AppError::BadRequest(format!(
                "file already exists: {filename}"
            )));
        }
        if dest_str != existing.path && FsPath::new(&existing.path).exists() {
            tokio::fs::remove_file(&existing.path).await?;
        }
        move_file(partial_path, &dest).await?;
        let meta = tokio::fs::metadata(&dest).await?;
        state.db.replace_platform_archive(
            archive_id,
            &dest_str,
            filename,
            meta.len() as i64,
        )?;
    } else if let Some(gid) = game_id {
        match upload_kind {
            UPLOAD_KIND_ARCHIVE => {
                let platform = platform
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| detect_platform_from_filename(filename).to_string());

                let existing = state
                    .db
                    .list_platform_archives(gid)?
                    .into_iter()
                    .find(|a| a.platform == platform);

                if let Some(existing) = existing {
                    if !is_path_under_archive_root(&existing.path, &settings.archive_path) {
                        return Err(AppError::BadRequest(
                            "existing archive is outside the configured folder".into(),
                        ));
                    }
                    if dest.exists() && dest_str != existing.path {
                        return Err(AppError::BadRequest(format!(
                            "file already exists: {filename}"
                        )));
                    }
                    if dest_str != existing.path && FsPath::new(&existing.path).exists() {
                        tokio::fs::remove_file(&existing.path).await?;
                    }
                    move_file(partial_path, &dest).await?;
                    let meta = tokio::fs::metadata(&dest).await?;
                    state.db.replace_platform_archive(
                        existing.id,
                        &dest_str,
                        filename,
                        meta.len() as i64,
                    )?;
                } else {
                    if dest.exists() {
                        return Err(AppError::BadRequest(format!(
                            "file already exists: {filename}"
                        )));
                    }
                    move_file(partial_path, &dest).await?;
                    let meta = tokio::fs::metadata(&dest).await?;
                    let is_default = state.db.list_platform_archives(gid)?.is_empty();
                    state.db.insert_platform_archive(
                        gid,
                        &platform,
                        &dest_str,
                        filename,
                        meta.len() as i64,
                        is_default,
                        Some(&now),
                    )?;
                }
            }
            UPLOAD_KIND_SAVE => {
                if dest.exists() {
                    tokio::fs::remove_file(&dest).await?;
                }
                move_file(partial_path, &dest).await?;
                let meta = tokio::fs::metadata(&dest).await?;
                state.db.insert_game_save(gid, &dest_str, filename, meta.len() as i64)?;
            }
            UPLOAD_KIND_PATCH => {
                if dest.exists() {
                    tokio::fs::remove_file(&dest).await?;
                }
                move_file(partial_path, &dest).await?;
                let meta = tokio::fs::metadata(&dest).await?;
                state
                    .db
                    .insert_game_patch(gid, &dest_str, filename, meta.len() as i64, None)?;
            }
            _ => {}
        }
    } else {
        if dest.exists() {
            return Err(AppError::BadRequest(format!(
                "file already exists: {filename}"
            )));
        }
        move_file(partial_path, &dest).await?;
        let meta = tokio::fs::metadata(&dest).await?;
        state
            .db
            .upsert_archive_file(&dest_str, filename, meta.len() as i64)?;
        if let Some(p) = platform {
            let _ = state.db.set_platform_for_path(&dest_str, p);
        }
    }

    state.db.delete_tus_upload(id)?;
    Ok(())
}
