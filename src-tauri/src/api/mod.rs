mod auth;
mod middleware;
mod tus;

use crate::error::AppResult;
use crate::models::{
    F95LoginRequest, F95LoginResult, GameDetail, GameResponse, MatchRequest, MigrationStatus,
    ReorganizeResult, ScanResult, SetArchivePlatformRequest, Settings, SetCoverRequest,
    StorageStats, UpdateGameUserDataRequest, UpdateSettingsRequest, VersionCheckResult,
};
use crate::state::SharedState;
use auth::{auth_status, login, logout};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, StatusCode},
    middleware::from_fn_with_state,
    response::{IntoResponse, Response},
    routing::{get, head, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::path::PathBuf;
use tokio_util::io::ReaderStream;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tus::{tus_create, tus_head, tus_options, tus_patch};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub tags: Option<String>,
    pub tags_mode: Option<String>,
    pub play_status: Option<String>,
    pub min_f95_rating: Option<f64>,
    pub max_f95_rating: Option<f64>,
    pub min_user_rating: Option<f64>,
    pub max_user_rating: Option<f64>,
    pub sort: Option<String>,
}

#[derive(Deserialize)]
pub struct F95SearchQuery {
    pub q: String,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct SuggestQuery {
    pub path: Option<String>,
    pub archive_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub archive_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct F95ThreadQuery {
    pub url: String,
}

pub fn create_router(state: SharedState, static_dir: Option<PathBuf>) -> Router {
    let protected = Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route("/settings/storage", get(get_storage_stats))
        .route("/settings/purge-media", post(purge_media_cache))
        .route("/auth/logout", post(logout))
        .route("/f95/login", post(f95_login))
        .route("/games", get(list_games))
        .route("/games/tags", get(list_library_tags))
        .route("/games/{id}", get(get_game))
        .route("/games/{id}/detail", get(get_game_detail))
        .route("/games/{id}/cover", post(set_game_cover))
        .route("/games/{id}/cover/reset", post(reset_game_cover))
        .route("/games/{id}/user-data", put(update_game_user_data))
        .route("/games/{id}/check-version", post(check_game_version))
        .route("/games/{id}/unmatch", post(unmatch_game))
        .route("/archives", get(list_archives))
        .route("/archives/scan", post(scan_archives))
        .route("/archives/suggest", get(suggest_matches))
        .route("/archives/migration", get(get_migration_status))
        .route("/archives/reorganize", post(reorganize_archives))
        .route("/archives/match", post(match_archive))
        .route(
            "/games/{id}/archives/{archive_id}/platform",
            put(set_archive_platform),
        )
        .route("/search/f95", get(search_f95))
        .route("/search/f95/thread", get(resolve_f95_thread))
        .route("/games/{id}/download", get(download_game))
        .route("/games/{id}/archives/{archive_id}/download", get(download_platform_archive))
        .route("/games/{id}/saves/{save_id}/download", get(download_game_save))
        .route("/games/{id}/patches/{patch_id}/download", get(download_game_patch))
        .route("/games/{id}/archive", axum::routing::delete(delete_archive))
        .route("/games/{id}/archives/{archive_id}", axum::routing::delete(delete_platform_archive))
        .route("/games/{id}/archives/{archive_id}/default", post(set_default_platform_archive))
        .route("/games/{id}/saves/{save_id}", axum::routing::delete(delete_game_save))
        .route("/games/{id}/patches/{patch_id}", axum::routing::delete(delete_game_patch))
        .route("/tus", post(tus_create).head(tus_options))
        .route("/tus/{id}", head(tus_head).patch(tus_patch))
        .layer(from_fn_with_state(state.clone(), middleware::auth_middleware));

    let api = Router::new()
        .route("/health", get(health))
        .route("/auth/status", get(auth_status))
        .route("/auth/login", post(login))
        .merge(protected)
        .with_state(state.clone());

    let mut router = Router::new().nest("/api", api);

    let media_dir = state.db.data_dir().join("media");
    if media_dir.exists() {
        let media = Router::new()
            .fallback_service(ServeDir::new(media_dir))
            .layer(from_fn_with_state(state.clone(), middleware::auth_middleware));
        router = router.nest("/api/media", media);
    }

    if let Some(static_dir) = static_dir {
        router = router.fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true));
    }

    router
        .layer(DefaultBodyLimit::disable())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn get_settings(State(state): State<SharedState>) -> AppResult<Json<Settings>> {
    Ok(Json(state.get_settings()?))
}

async fn update_settings(
    State(state): State<SharedState>,
    Json(req): Json<UpdateSettingsRequest>,
) -> AppResult<Json<Settings>> {
    Ok(Json(state.update_settings(req).await?))
}

async fn purge_media_cache(State(state): State<SharedState>) -> AppResult<impl IntoResponse> {
    state.purge_media_cache()?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn f95_login(
    State(state): State<SharedState>,
    Json(req): Json<F95LoginRequest>,
) -> AppResult<Json<F95LoginResult>> {
    Ok(Json(state.f95_login(req).await?))
}

async fn list_games(
    State(state): State<SharedState>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<GameResponse>>> {
    let games = state.list_games(
        query.q,
        query.tags,
        query.tags_mode,
        query.play_status,
        query.min_f95_rating,
        query.max_f95_rating,
        query.min_user_rating,
        query.max_user_rating,
        query.sort,
    )?;
    let response = games
        .into_iter()
        .map(|game| state.game_response(game))
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Json(response))
}

async fn list_library_tags(
    State(state): State<SharedState>,
) -> AppResult<impl IntoResponse> {
    Ok(Json(state.list_library_tags()?))
}

async fn get_game(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<GameResponse>> {
    let game = state.get_game(id)?;
    Ok(Json(state.game_response(game)?))
}

async fn set_game_cover(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
    Json(req): Json<SetCoverRequest>,
) -> AppResult<Json<GameResponse>> {
    Ok(Json(state.set_game_cover(id, req.screenshot_index)?))
}

async fn reset_game_cover(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<GameResponse>> {
    Ok(Json(state.reset_game_cover(id)?))
}

async fn update_game_user_data(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateGameUserDataRequest>,
) -> AppResult<Json<GameResponse>> {
    let game = state.update_game_user_data(id, req)?;
    Ok(Json(state.game_response(game)?))
}

async fn get_storage_stats(
    State(state): State<SharedState>,
) -> AppResult<Json<StorageStats>> {
    Ok(Json(state.get_storage_stats()?))
}

async fn get_game_detail(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<GameDetail>> {
    Ok(Json(state.get_game_detail(id)?))
}

async fn check_game_version(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<VersionCheckResult>> {
    Ok(Json(state.check_game_version_update(id).await?))
}

async fn unmatch_game(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<GameResponse>> {
    let game = state.unmatch_archive(id)?;
    Ok(Json(state.game_response(game)?))
}

async fn list_archives(State(state): State<SharedState>) -> AppResult<impl IntoResponse> {
    Ok(Json(state.list_archives()?))
}

async fn scan_archives(State(state): State<SharedState>) -> AppResult<Json<ScanResult>> {
    Ok(Json(state.scan_archives().await?))
}

async fn search_f95(
    State(state): State<SharedState>,
    Query(query): Query<F95SearchQuery>,
) -> AppResult<impl IntoResponse> {
    let page = query.page.unwrap_or(1);
    Ok(Json(state.search_f95(&query.q, page).await?))
}

async fn resolve_f95_thread(
    State(state): State<SharedState>,
    Query(query): Query<F95ThreadQuery>,
) -> AppResult<impl IntoResponse> {
    Ok(Json(state.resolve_f95_thread(&query.url).await?))
}

async fn suggest_matches(
    State(state): State<SharedState>,
    Query(query): Query<SuggestQuery>,
) -> AppResult<impl IntoResponse> {
    Ok(Json(state.suggest_matches(query.archive_id, query.path.as_deref()).await?))
}

async fn match_archive(
    State(state): State<SharedState>,
    Json(req): Json<MatchRequest>,
) -> AppResult<Json<GameResponse>> {
    let game = state.match_archive(req).await?;
    Ok(Json(state.game_response(game)?))
}

async fn get_migration_status(
    State(state): State<SharedState>,
) -> AppResult<Json<MigrationStatus>> {
    Ok(Json(state.get_migration_status()?))
}

async fn reorganize_archives(
    State(state): State<SharedState>,
) -> AppResult<Json<ReorganizeResult>> {
    Ok(Json(state.reorganize_legacy_archives()?))
}

async fn set_archive_platform(
    State(state): State<SharedState>,
    Path((game_id, archive_id)): Path<(i64, i64)>,
    Json(req): Json<SetArchivePlatformRequest>,
) -> AppResult<impl IntoResponse> {
    let archive = state.db.get_platform_archive(archive_id)?;
    if archive.game_id != game_id {
        return Err(crate::error::AppError::NotFound("archive not found".into()));
    }
    let updated = state.assign_archive_platform(archive_id, &req.platform, req.reorganize)?;
    Ok(Json(updated))
}

async fn delete_archive(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    state.delete_archive(id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn download_game(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
    Query(query): Query<DownloadQuery>,
) -> AppResult<Response> {
    if let Some(archive_id) = query.archive_id {
        return download_platform_archive(State(state), Path((id, archive_id))).await;
    }
    let game = state.get_game(id)?;
    stream_attachment(&game.archive_path, &game.archive_filename).await
}

async fn download_platform_archive(
    State(state): State<SharedState>,
    Path((game_id, archive_id)): Path<(i64, i64)>,
) -> AppResult<Response> {
    let archive = state.db.get_platform_archive(archive_id)?;
    if archive.game_id != game_id {
        return Err(crate::error::AppError::NotFound("archive not found".into()));
    }
    stream_attachment(&archive.path, &archive.filename).await
}

async fn download_game_save(
    State(state): State<SharedState>,
    Path((game_id, save_id)): Path<(i64, i64)>,
) -> AppResult<Response> {
    let save = state.db.get_game_save(save_id)?;
    if save.game_id != game_id {
        return Err(crate::error::AppError::NotFound("save not found".into()));
    }
    stream_attachment(&save.path, &save.filename).await
}

async fn download_game_patch(
    State(state): State<SharedState>,
    Path((game_id, patch_id)): Path<(i64, i64)>,
) -> AppResult<Response> {
    let patch = state.db.get_game_patch(patch_id)?;
    if patch.game_id != game_id {
        return Err(crate::error::AppError::NotFound("patch not found".into()));
    }
    stream_attachment(&patch.path, &patch.filename).await
}

async fn stream_attachment(path: &str, filename: &str) -> AppResult<Response> {
    let file = tokio::fs::File::open(path).await.map_err(|e| {
        crate::error::AppError::NotFound(format!("file not found: {e}"))
    })?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(body)
        .unwrap())
}

async fn delete_platform_archive(
    State(state): State<SharedState>,
    Path((game_id, archive_id)): Path<(i64, i64)>,
) -> AppResult<impl IntoResponse> {
    let archive = state.db.get_platform_archive(archive_id)?;
    if archive.game_id != game_id {
        return Err(crate::error::AppError::NotFound("archive not found".into()));
    }
    state.delete_platform_archive(archive_id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn set_default_platform_archive(
    State(state): State<SharedState>,
    Path((game_id, archive_id)): Path<(i64, i64)>,
) -> AppResult<Json<GameResponse>> {
    let archive = state.db.get_platform_archive(archive_id)?;
    if archive.game_id != game_id {
        return Err(crate::error::AppError::NotFound("archive not found".into()));
    }
    let game = state.set_default_platform_archive(archive_id)?;
    Ok(Json(state.game_response(game)?))
}

async fn delete_game_save(
    State(state): State<SharedState>,
    Path((game_id, save_id)): Path<(i64, i64)>,
) -> AppResult<impl IntoResponse> {
    let save = state.db.get_game_save(save_id)?;
    if save.game_id != game_id {
        return Err(crate::error::AppError::NotFound("save not found".into()));
    }
    state.delete_game_save(save_id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn delete_game_patch(
    State(state): State<SharedState>,
    Path((game_id, patch_id)): Path<(i64, i64)>,
) -> AppResult<impl IntoResponse> {
    let patch = state.db.get_game_patch(patch_id)?;
    if patch.game_id != game_id {
        return Err(crate::error::AppError::NotFound("patch not found".into()));
    }
    state.delete_game_patch(patch_id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn run_server(
    host: &str,
    port: u16,
    data_dir: std::path::PathBuf,
    static_dir: Option<PathBuf>,
) -> AppResult<()> {
    let state = SharedState::new(crate::state::AppState::new(&data_dir)?);
    let router = create_router(state, static_dir);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("AVN Hub server listening on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
}
