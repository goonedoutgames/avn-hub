use crate::api::auth::require_session;
use crate::error::{AppError, AppResult};
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
            _ => {}
        }
    }

    Some(UploadMetadata {
        filename: filename?,
        replace_game_id,
    })
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
    let filename = sanitize_archive_filename(&parsed.filename)
        .ok_or_else(|| AppError::BadRequest("invalid archive filename".into()))?;

    let settings = state.get_settings()?;
    if settings.archive_path.trim().is_empty() {
        return Err(AppError::BadRequest(
            "archive path not configured. Set it in Settings first.".into(),
        ));
    }

    if let Some(game_id) = parsed.replace_game_id {
        state.db.get_game(game_id).map_err(|_| {
            AppError::BadRequest(format!("game {game_id} not found for archive replace"))
        })?;
    }

    let id = uuid::Uuid::new_v4().to_string();
    let uploads_dir = state.db.uploads_dir();
    tokio::fs::create_dir_all(&uploads_dir).await?;
    let partial_path = uploads_dir.join(format!("{id}.partial"));
    tokio::fs::File::create(&partial_path).await?;

    state
        .db
        .create_tus_upload(&id, &filename, length, parsed.replace_game_id)?;

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

    let (_, size, offset, _) = state
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

    let (filename, size, stored_offset, replace_game_id) = state
        .db
        .get_tus_upload(&id)?
        .ok_or_else(|| AppError::NotFound("upload not found".into()))?;

    if offset_header != stored_offset {
        return Err(AppError::BadRequest(format!(
            "offset mismatch: expected {stored_offset}, got {offset_header}"
        )));
    }

    let partial_path = state.db.uploads_dir().join(format!("{id}.partial"));
    let mut file = tokio::fs::OpenOptions::new()
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
    state.db.update_tus_offset(&id, new_offset)?;

    if new_offset == size {
        finalize_upload(
            &state,
            &id,
            &filename,
            &partial_path,
            replace_game_id,
        )
        .await?;
    }

    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Upload-Offset", new_offset.to_string())
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
    replace_game_id: Option<i64>,
) -> AppResult<()> {
    let settings = state.get_settings()?;
    let dest_dir = FsPath::new(&settings.archive_path);
    tokio::fs::create_dir_all(dest_dir).await?;
    let dest = dest_dir.join(filename);
    let dest_str = dest.to_string_lossy().to_string();

    if let Some(game_id) = replace_game_id {
        let game = state.db.get_game(game_id)?;
        if !is_path_under_archive_root(&game.archive_path, &settings.archive_path) {
            return Err(AppError::BadRequest(
                "existing archive is outside the configured folder".into(),
            ));
        }
        if dest.exists() && dest_str != game.archive_path {
            return Err(AppError::BadRequest(format!(
                "file already exists: {filename}"
            )));
        }
        if dest_str != game.archive_path && FsPath::new(&game.archive_path).exists() {
            tokio::fs::remove_file(&game.archive_path).await?;
        }
        tokio::fs::rename(partial_path, &dest).await?;
        let meta = tokio::fs::metadata(&dest).await?;
        state.db.replace_game_archive(
            game_id,
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
        tokio::fs::rename(partial_path, &dest).await?;
        let meta = tokio::fs::metadata(&dest).await?;
        state.db.upsert_archive_file(&dest_str, filename, meta.len() as i64)?;
    }

    state.db.delete_tus_upload(id)?;
    Ok(())
}
