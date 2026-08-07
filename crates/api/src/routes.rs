use crate::{ApiError, ApiResult, ApiState, AuthToken, RequireAuth};
use avn_hub_auth::AuthService;
use avn_hub_core::{
    AttachmentKind, LibraryFilter, LibrarySort, TagMode, UpdateGameUserData,
};
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/health", get(crate::health))
        .route("/api/v1/health", get(crate::health))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/settings", get(get_settings).put(update_settings))
        .route("/api/v1/settings/f95/login", post(f95_login))
        .route("/api/v1/settings/f95/cookies", post(f95_cookies))
        .route("/api/v1/settings/storage", get(storage_stats))
        .route("/api/v1/settings/media/purge", post(purge_media))
        .route("/api/v1/catalog/search", get(catalog_search))
        .route("/api/v1/catalog/browse", get(catalog_browse))
        .route("/api/v1/catalog/preview", get(catalog_preview))
        .route("/api/v1/catalog/tags", get(catalog_tags))
        .route("/api/v1/library", get(list_library))
        .route("/api/v1/library/tags", get(library_tags))
        .route("/api/v1/library/add", post(add_game))
        .route("/api/v1/library/check-updates", post(check_all_updates))
        .route(
            "/api/v1/games/{id}",
            get(get_game).patch(patch_game).delete(delete_game),
        )
        .route("/api/v1/games/{id}/refresh", post(refresh_game))
        .route("/api/v1/games/{id}/check-version", post(check_version))
        .route("/api/v1/games/{id}/cover", post(set_cover))
        .route("/api/v1/games/{id}/cover/reset", post(reset_cover))
        .route("/api/v1/games/{id}/saves", get(list_saves).post(upload_save))
        .route(
            "/api/v1/games/{id}/saves/{save_id}",
            get(download_save).delete(delete_save),
        )
        .route(
            "/api/v1/games/{id}/patches",
            get(list_patches).post(upload_patch),
        )
        .route(
            "/api/v1/games/{id}/patches/{patch_id}",
            get(download_patch).delete(delete_patch),
        )
        .route("/api/v1/media/{*path}", get(serve_media))
}

#[derive(Deserialize)]
struct LoginBody {
    password: String,
}

async fn login(
    State(state): State<ApiState>,
    Json(body): Json<LoginBody>,
) -> ApiResult<impl IntoResponse> {
    let token = AuthService::login(&state.app.db, &body.password)?;
    Ok(Json(json!({ "token": token })))
}

async fn logout(
    State(state): State<ApiState>,
    AuthToken(token): AuthToken,
) -> ApiResult<impl IntoResponse> {
    if let Some(token) = token {
        AuthService::logout(&state.app.db, &token)?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn me(
    State(state): State<ApiState>,
    AuthToken(token): AuthToken,
) -> ApiResult<impl IntoResponse> {
    let configured = AuthService::is_configured(&state.app.db)?;
    let authenticated = if !configured {
        true
    } else {
        match token.as_deref() {
            Some(t) => AuthService::validate(&state.app.db, t)?,
            None => false,
        }
    };
    Ok(Json(json!({
        "configured": configured,
        "authenticated": authenticated,
    })))
}

async fn get_settings(
    State(state): State<ApiState>,
    _: RequireAuth,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.settings_view().await?))
}

#[derive(Deserialize)]
struct UpdateSettingsBody {
    app_password: Option<String>,
    app_password_remove: Option<bool>,
    f95_username: Option<String>,
    f95_password: Option<String>,
    max_attachment_bytes: Option<u64>,
    tag_click_action: Option<String>,
}

async fn update_settings(
    State(state): State<ApiState>,
    _: RequireAuth,
    Json(body): Json<UpdateSettingsBody>,
) -> ApiResult<impl IntoResponse> {
    if body.app_password_remove.unwrap_or(false) {
        AuthService::remove_password(&state.app.db)?;
    }
    if let Some(pw) = body.app_password.filter(|p| !p.is_empty()) {
        AuthService::set_password(&state.app.db, &pw)?;
    }
    if let Some(user) = body.f95_username {
        state.app.db.set_setting("f95_username", &user)?;
    }
    if let Some(pass) = body.f95_password.filter(|p| !p.is_empty()) {
        state.app.db.set_setting("f95_password", &pass)?;
    }
    if let Some(max) = body.max_attachment_bytes {
        state
            .app
            .db
            .set_setting("max_attachment_bytes", &max.to_string())?;
    }
    if let Some(action) = body.tag_click_action {
        state.app.set_tag_click_action(&action)?;
    }
    Ok(Json(state.app.settings_view().await?))
}

#[derive(Deserialize)]
struct F95LoginBody {
    username: String,
    password: String,
}

async fn f95_login(
    State(state): State<ApiState>,
    _: RequireAuth,
    Json(body): Json<F95LoginBody>,
) -> ApiResult<impl IntoResponse> {
    let message = state.app.f95_login(&body.username, &body.password).await?;
    Ok(Json(json!({ "ok": true, "message": message })))
}

#[derive(Deserialize)]
struct F95CookiesBody {
    cookies: String,
}

async fn f95_cookies(
    State(state): State<ApiState>,
    _: RequireAuth,
    Json(body): Json<F95CookiesBody>,
) -> ApiResult<impl IntoResponse> {
    let message = state.app.f95_set_cookies(&body.cookies).await?;
    Ok(Json(json!({ "ok": true, "message": message })))
}

async fn storage_stats(
    State(state): State<ApiState>,
    _: RequireAuth,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.storage_stats()?))
}

async fn purge_media(
    State(state): State<ApiState>,
    _: RequireAuth,
) -> ApiResult<impl IntoResponse> {
    state.app.purge_media_cache()?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct CatalogQuery {
    q: Option<String>,
    search: Option<String>,
    creator: Option<String>,
    page: Option<u32>,
    sort: Option<String>,
    /// Updated within N days (F95 `date` param). 0 = any.
    date: Option<u32>,
    tag_mode: Option<String>,
    tags: Option<String>,
    notags: Option<String>,
    prefixes: Option<String>,
}

fn split_csv(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

async fn catalog_search(
    State(state): State<ApiState>,
    _: RequireAuth,
    Query(q): Query<CatalogQuery>,
) -> ApiResult<impl IntoResponse> {
    let search = q
        .search
        .or(q.q)
        .unwrap_or_default();
    let filter = avn_hub_core::f95zone::CatalogFilter {
        search,
        creator: q.creator.unwrap_or_default(),
        page: q.page.unwrap_or(1),
        rows: 30,
        sort: q.sort.unwrap_or_else(|| "date".into()),
        date_days: q.date.unwrap_or(0),
        tag_mode: q.tag_mode.unwrap_or_else(|| "and".into()),
        tags: split_csv(q.tags),
        notags: split_csv(q.notags),
        prefixes: split_csv(q.prefixes),
    };
    Ok(Json(state.app.catalog_search(filter).await?))
}

async fn catalog_browse(
    State(state): State<ApiState>,
    _: RequireAuth,
    Query(q): Query<CatalogQuery>,
) -> ApiResult<impl IntoResponse> {
    let filter = avn_hub_core::f95zone::CatalogFilter {
        search: q.search.or(q.q).unwrap_or_default(),
        creator: q.creator.unwrap_or_default(),
        page: q.page.unwrap_or(1),
        rows: 30,
        sort: q.sort.unwrap_or_else(|| "date".into()),
        date_days: q.date.unwrap_or(0),
        tag_mode: q.tag_mode.unwrap_or_else(|| "and".into()),
        tags: split_csv(q.tags),
        notags: split_csv(q.notags),
        prefixes: split_csv(q.prefixes),
    };
    Ok(Json(state.app.catalog_search(filter).await?))
}

#[derive(Deserialize)]
struct PreviewQuery {
    input: String,
}

async fn catalog_preview(
    State(state): State<ApiState>,
    _: RequireAuth,
    Query(q): Query<PreviewQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.preview_thread(&q.input).await?))
}

#[derive(Deserialize)]
struct CatalogTagsQuery {
    q: Option<String>,
    limit: Option<usize>,
}

async fn catalog_tags(
    State(state): State<ApiState>,
    _: RequireAuth,
    Query(q): Query<CatalogTagsQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        state
            .app
            .catalog_tags(q.q.as_deref(), q.limit.unwrap_or(200))
            .await?,
    ))
}

#[derive(Deserialize)]
struct LibraryQuery {
    search: Option<String>,
    play_status: Option<String>,
    /// Minimum user rating filter, or the string "unrated".
    user_rating: Option<String>,
    user_rating_min: Option<f64>,
    tags: Option<String>,
    tag_mode: Option<String>,
    sort: Option<String>,
}

async fn list_library(
    State(state): State<ApiState>,
    _: RequireAuth,
    Query(q): Query<LibraryQuery>,
) -> ApiResult<impl IntoResponse> {
    let unrated_only = q
        .user_rating
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("unrated"));
    let user_rating_min = if unrated_only {
        None
    } else {
        q.user_rating_min.or_else(|| {
            q.user_rating
                .as_ref()
                .and_then(|v| v.trim().parse::<f64>().ok())
        })
    };

    let filter = LibraryFilter {
        search: q.search,
        play_status: q.play_status,
        user_rating_min,
        unrated_only,
        tags: q
            .tags
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        tag_mode: match q.tag_mode.as_deref() {
            Some("or") => TagMode::Or,
            _ => TagMode::And,
        },
        sort: match q.sort.as_deref() {
            Some("title_desc") => LibrarySort::TitleDesc,
            Some("updated_desc") => LibrarySort::UpdatedDesc,
            Some("rating_desc") => LibrarySort::RatingDesc,
            Some("user_rating_desc") => LibrarySort::UserRatingDesc,
            _ => LibrarySort::TitleAsc,
        },
    };
    Ok(Json(state.app.list_library(&filter)?))
}

async fn library_tags(
    State(state): State<ApiState>,
    _: RequireAuth,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.db.list_library_tags()?))
}

#[derive(Deserialize)]
struct AddGameBody {
    input: String,
}

async fn add_game(
    State(state): State<ApiState>,
    _: RequireAuth,
    Json(body): Json<AddGameBody>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.add_game_from_f95(&body.input).await?))
}

async fn check_all_updates(
    State(state): State<ApiState>,
    _: RequireAuth,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.check_all_versions().await?))
}

async fn get_game(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.game_detail(id)?))
}

async fn patch_game(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
    Json(body): Json<UpdateGameUserData>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.update_user_data(id, body)?))
}

async fn delete_game(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    state.app.delete_game(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn refresh_game(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.refresh_game_metadata(id).await?))
}

async fn check_version(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.check_version(id).await?))
}

#[derive(Deserialize)]
struct SetCoverBody {
    screenshot_index: usize,
}

async fn set_cover(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
    Json(body): Json<SetCoverBody>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        state
            .app
            .set_cover_from_screenshot(id, body.screenshot_index)?,
    ))
}

async fn reset_cover(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.reset_cover(id)?))
}

async fn list_saves(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.db.list_saves(id)?))
}

async fn list_patches(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(state.app.db.list_patches(id)?))
}

async fn upload_save(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> ApiResult<impl IntoResponse> {
    let (filename, bytes, _) = read_multipart_file(multipart).await?;
    let save_id = state.app.save_attachment_bytes(
        id,
        AttachmentKind::Save,
        &filename,
        &bytes,
        None,
    )?;
    Ok(Json(state.app.db.get_save(save_id)?))
}

async fn upload_patch(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> ApiResult<impl IntoResponse> {
    let (filename, bytes, description) = read_multipart_file(multipart).await?;
    let patch_id = state.app.save_attachment_bytes(
        id,
        AttachmentKind::Patch,
        &filename,
        &bytes,
        description.as_deref(),
    )?;
    Ok(Json(state.app.db.get_patch(patch_id)?))
}

async fn download_save(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path((_game_id, save_id)): Path<(i64, i64)>,
) -> ApiResult<Response> {
    let save = state.app.db.get_save(save_id)?;
    file_response(&save.path, &save.filename)
}

async fn delete_save(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path((_game_id, save_id)): Path<(i64, i64)>,
) -> ApiResult<impl IntoResponse> {
    state.app.delete_save(save_id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn download_patch(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path((_game_id, patch_id)): Path<(i64, i64)>,
) -> ApiResult<Response> {
    let patch = state.app.db.get_patch(patch_id)?;
    file_response(&patch.path, &patch.filename)
}

async fn delete_patch(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path((_game_id, patch_id)): Path<(i64, i64)>,
) -> ApiResult<impl IntoResponse> {
    state.app.delete_patch(patch_id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn serve_media(
    State(state): State<ApiState>,
    _: RequireAuth,
    Path(path): Path<String>,
) -> ApiResult<Response> {
    let file = state.app.resolve_media_file(&path)?;
    let mime = mime_guess::from_path(&file)
        .first_or_octet_stream()
        .to_string();
    let data = tokio::fs::read(&file)
        .await
        .map_err(|e| ApiError(avn_hub_core::AppError::from(e)))?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(data))
        .unwrap())
}

async fn read_multipart_file(
    mut multipart: Multipart,
) -> ApiResult<(String, Vec<u8>, Option<String>)> {
    let mut filename = String::from("upload.bin");
    let mut bytes = Vec::new();
    let mut description = None;
    let mut fields: HashMap<String, String> = HashMap::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(avn_hub_core::AppError::BadRequest(e.to_string())))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || field.file_name().is_some() {
            if let Some(name) = field.file_name() {
                filename = name.to_string();
            }
            bytes = field
                .bytes()
                .await
                .map_err(|e| ApiError(avn_hub_core::AppError::BadRequest(e.to_string())))?
                .to_vec();
        } else {
            let text = field
                .text()
                .await
                .map_err(|e| ApiError(avn_hub_core::AppError::BadRequest(e.to_string())))?;
            fields.insert(name, text);
        }
    }

    if bytes.is_empty() {
        return Err(ApiError(avn_hub_core::AppError::BadRequest(
            "No file uploaded".into(),
        )));
    }

    description = fields.get("description").cloned();
    Ok((filename, bytes, description))
}

fn file_response(path: &str, filename: &str) -> ApiResult<Response> {
    let data = std::fs::read(path).map_err(|e| ApiError(avn_hub_core::AppError::from(e)))?;
    let mime = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(data))
        .unwrap())
}
