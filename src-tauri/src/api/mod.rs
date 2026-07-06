use crate::error::AppResult;
use crate::models::{
    F95LoginRequest, F95LoginResult, GameDetail, GameResponse, MatchRequest, ScanResult, Settings,
    SetCoverRequest, UpdateSettingsRequest,
};
use crate::state::SharedState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tokio_util::io::ReaderStream;
use serde::Deserialize;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub tags: Option<String>,
    pub tags_mode: Option<String>,
}

#[derive(Deserialize)]
pub struct F95SearchQuery {
    pub q: String,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct SuggestQuery {
    pub path: String,
}

pub fn create_router(state: SharedState, static_dir: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/settings", get(get_settings).put(update_settings))
        .route("/settings/purge-media", post(purge_media_cache))
        .route("/f95/login", post(f95_login))
        .route("/games", get(list_games))
        .route("/games/tags", get(list_library_tags))
        .route("/games/{id}", get(get_game))
        .route("/games/{id}/detail", get(get_game_detail))
        .route("/games/{id}/cover", post(set_game_cover))
        .route("/games/{id}/unmatch", post(unmatch_game))
        .route("/archives", get(list_archives))
        .route("/archives/scan", post(scan_archives))
        .route("/archives/suggest", get(suggest_matches))
        .route("/archives/match", post(match_archive))
        .route("/search/f95", get(search_f95))
        .route("/games/{id}/download", get(download_game))
        .with_state(state.clone());

    let mut router = Router::new().nest("/api", api);

    let media_dir = state.db.data_dir().join("media");
    if media_dir.exists() {
        router = router.nest_service("/api/media", ServeDir::new(media_dir));
    }

    if let Some(static_dir) = static_dir {
        router = router.fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true));
    }

    router
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
    let games = state.list_games(query.q, query.tags, query.tags_mode)?;
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

async fn get_game_detail(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<GameDetail>> {
    Ok(Json(state.get_game_detail(id)?))
}

async fn unmatch_game(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Json<GameResponse>> {
    let game = state.unmatch_archive(id)?;
    Ok(Json(GameResponse {
        game,
        cover_url: None,
        cover_full_url: None,
        preview_urls: vec![],
    }))
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

async fn suggest_matches(
    State(state): State<SharedState>,
    Query(query): Query<SuggestQuery>,
) -> AppResult<impl IntoResponse> {
    Ok(Json(state.suggest_matches(&query.path).await?))
}

async fn match_archive(
    State(state): State<SharedState>,
    Json(req): Json<MatchRequest>,
) -> AppResult<Json<GameResponse>> {
    let game = state.match_archive(req).await?;
    Ok(Json(state.game_response(game)?))
}

async fn download_game(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let game = state.get_game(id)?;
    let file = tokio::fs::File::open(&game.archive_path).await.map_err(|e| {
        crate::error::AppError::NotFound(format!("archive file not found: {e}"))
    })?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", game.archive_filename),
        )
        .body(body)
        .unwrap())
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
