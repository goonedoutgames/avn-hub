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
    play_status: Option<String>,
    min_f95_rating: Option<f64>,
    max_f95_rating: Option<f64>,
    min_user_rating: Option<f64>,
    max_user_rating: Option<f64>,
    sort: Option<String>,
) -> AppResult<Vec<GameResponse>> {
    let games = state.list_games(
        search,
        tags,
        tags_mode,
        play_status,
        min_f95_rating,
        max_f95_rating,
        min_user_rating,
        max_user_rating,
        sort,
    )?;
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
    state.game_response(game)
}

#[tauri::command]
pub async fn reset_game_cover(
    state: State<'_, SharedState>,
    id: i64,
) -> AppResult<GameResponse> {
    state.reset_game_cover(id)
}

#[tauri::command]
pub async fn update_game_user_data(
    state: State<'_, SharedState>,
    id: i64,
    req: crate::models::UpdateGameUserDataRequest,
) -> AppResult<GameResponse> {
    let game = state.update_game_user_data(id, req)?;
    state.game_response(game)
}

#[tauri::command]
pub async fn get_storage_stats(
    state: State<'_, SharedState>,
) -> AppResult<crate::models::StorageStats> {
    state.get_storage_stats()
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
    archive_id: Option<i64>,
    archive_path: Option<String>,
) -> AppResult<Vec<crate::models::F95SearchResult>> {
    state
        .suggest_matches(archive_id, archive_path.as_deref())
        .await
}

#[tauri::command]
pub async fn delete_platform_archive(
    state: State<'_, SharedState>,
    archive_id: i64,
) -> AppResult<()> {
    state.delete_platform_archive(archive_id)
}

#[tauri::command]
pub async fn set_default_platform_archive(
    state: State<'_, SharedState>,
    archive_id: i64,
) -> AppResult<GameResponse> {
    let game = state.set_default_platform_archive(archive_id)?;
    game_with_cover(&state, game)
}

#[tauri::command]
pub async fn delete_game_save(
    state: State<'_, SharedState>,
    save_id: i64,
) -> AppResult<()> {
    state.delete_game_save(save_id)
}

#[tauri::command]
pub async fn delete_game_patch(
    state: State<'_, SharedState>,
    patch_id: i64,
) -> AppResult<()> {
    state.delete_game_patch(patch_id)
}

#[tauri::command]
pub async fn check_game_version(
    state: State<'_, SharedState>,
    id: i64,
) -> AppResult<crate::models::VersionCheckResult> {
    state.check_game_version_update(id).await
}

#[tauri::command]
pub async fn get_migration_status(
    state: State<'_, SharedState>,
) -> AppResult<crate::models::MigrationStatus> {
    state.get_migration_status()
}

#[tauri::command]
pub async fn reorganize_archives(
    state: State<'_, SharedState>,
) -> AppResult<crate::models::ReorganizeResult> {
    state.reorganize_legacy_archives()
}

#[tauri::command]
pub async fn assign_archive_platform(
    state: State<'_, SharedState>,
    archive_id: i64,
    platform: String,
    reorganize: Option<bool>,
) -> AppResult<crate::models::GamePlatformArchive> {
    state.assign_archive_platform(archive_id, &platform, reorganize.unwrap_or(true))
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
    archive_id: Option<i64>,
) -> AppResult<String> {
    let (path, filename) = if let Some(aid) = archive_id {
        let archive = state.db.get_platform_archive(aid)?;
        if archive.game_id != game_id {
            return Err(crate::error::AppError::NotFound("archive not found".into()));
        }
        (archive.path, archive.filename)
    } else {
        let game = state.get_game(game_id)?;
        (game.archive_path, game.archive_filename)
    };

    let dest = rfd::AsyncFileDialog::new()
        .set_file_name(&filename)
        .add_filter("Game file", &["zip", "rar", "7z", "bz2", "apk", "xapk", "apks"])
        .save_file()
        .await;

    let Some(dest) = dest else {
        return Err(crate::error::AppError::BadRequest("download cancelled".into()));
    };

    tokio::fs::copy(&path, dest.path()).await?;
    Ok(dest.path().display().to_string())
}
