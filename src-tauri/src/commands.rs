use crate::error::AppResult;
use crate::models::{
    F95LoginRequest, F95LoginResult, GameDetail, GameResponse, MatchRequest, ScanResult, Settings,
    UpdateSettingsRequest,
};
use crate::state::{AppState, SharedState};
use tauri::State;

fn game_with_cover(state: &AppState, game: crate::models::Game) -> AppResult<GameResponse> {
    state.game_response(game)
}

#[tauri::command]
pub async fn get_settings(state: State<'_, SharedState>) -> AppResult<Settings> {
    state.get_settings()
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, SharedState>,
    req: UpdateSettingsRequest,
) -> AppResult<Settings> {
    state.update_settings(req).await
}

#[tauri::command]
pub async fn f95_login(
    state: State<'_, SharedState>,
    req: F95LoginRequest,
) -> AppResult<F95LoginResult> {
    state.f95_login(req).await
}

#[tauri::command]
pub async fn list_games(
    state: State<'_, SharedState>,
    search: Option<String>,
    tags: Option<String>,
    tags_mode: Option<String>,
) -> AppResult<Vec<GameResponse>> {
    let games = state.list_games(search, tags, tags_mode)?;
    games
        .into_iter()
        .map(|g| game_with_cover(&state, g))
        .collect()
}

#[tauri::command]
pub async fn get_game(state: State<'_, SharedState>, id: i64) -> AppResult<GameResponse> {
    let game = state.get_game(id)?;
    game_with_cover(&state, game)
}

#[tauri::command]
pub async fn get_game_detail(
    state: State<'_, SharedState>,
    id: i64,
) -> AppResult<GameDetail> {
    state.get_game_detail(id)
}

#[tauri::command]
pub async fn delete_archive(state: State<'_, SharedState>, game_id: i64) -> AppResult<()> {
    state.delete_archive(game_id)
}

#[tauri::command]
pub async fn unmatch_game(state: State<'_, SharedState>, id: i64) -> AppResult<GameResponse> {
    let game = state.unmatch_archive(id)?;
    Ok(GameResponse {
        game,
        cover_url: None,
        cover_full_url: None,
        preview_urls: vec![],
    })
}

#[tauri::command]
pub async fn set_game_cover(
    state: State<'_, SharedState>,
    id: i64,
    screenshot_index: usize,
) -> AppResult<GameResponse> {
    state.set_game_cover(id, screenshot_index)
}

#[tauri::command]
pub async fn list_library_tags(
    state: State<'_, SharedState>,
) -> AppResult<Vec<crate::models::LibraryTag>> {
    state.list_library_tags()
}

#[tauri::command]
pub async fn purge_media_cache(state: State<'_, SharedState>) -> AppResult<()> {
    state.purge_media_cache()
}

#[tauri::command]
pub async fn list_archives(
    state: State<'_, SharedState>,
) -> AppResult<Vec<crate::models::ArchiveEntry>> {
    state.list_archives()
}

#[tauri::command]
pub async fn scan_archives(state: State<'_, SharedState>) -> AppResult<ScanResult> {
    state.scan_archives().await
}

#[tauri::command]
pub async fn search_f95(
    state: State<'_, SharedState>,
    query: String,
    page: Option<u32>,
) -> AppResult<Vec<crate::models::F95SearchResult>> {
    state.search_f95(&query, page.unwrap_or(1)).await
}

#[tauri::command]
pub async fn resolve_f95_thread(
    state: State<'_, SharedState>,
    url: String,
) -> AppResult<crate::models::F95SearchResult> {
    state.resolve_f95_thread(&url).await
}

#[tauri::command]
pub async fn suggest_matches(
    state: State<'_, SharedState>,
    archive_path: String,
) -> AppResult<Vec<crate::models::F95SearchResult>> {
    state.suggest_matches(&archive_path).await
}

#[tauri::command]
pub async fn match_archive(
    state: State<'_, SharedState>,
    req: MatchRequest,
) -> AppResult<GameResponse> {
    let game = state.match_archive(req).await?;
    game_with_cover(&state, game)
}

#[tauri::command]
pub async fn get_media_path(
    state: State<'_, SharedState>,
    game_id: i64,
) -> AppResult<Option<String>> {
    let game = state.get_game(game_id)?;
    Ok(game.cover_image_path)
}

#[tauri::command]
pub async fn download_game(
    state: State<'_, SharedState>,
    game_id: i64,
) -> AppResult<String> {
    let game = state.get_game(game_id)?;
    let dest = rfd::AsyncFileDialog::new()
        .set_file_name(&game.archive_filename)
        .add_filter("Archive", &["zip", "rar", "7z", "bz2"])
        .save_file()
        .await;

    let Some(dest) = dest else {
        return Err(crate::error::AppError::BadRequest("download cancelled".into()));
    };

    tokio::fs::copy(&game.archive_path, dest.path()).await?;
    Ok(dest.path().display().to_string())
}
