pub mod api;
mod commands;
pub mod db;
pub mod error;
pub mod http_auth;
pub mod models;
mod scanner;
mod sources;
pub mod state;
mod storage;
mod migration;
mod platform;
mod attachments;

use db::default_data_dir;
use state::{AppState, SharedState};

pub use api::run_server;
pub use error::{AppError, AppResult};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = default_data_dir();
    let app_state = AppState::new(&data_dir).expect("failed to initialize database");
    let shared_state = SharedState::new(app_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(shared_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::purge_media_cache,
            commands::f95_login,
            commands::list_games,
            commands::list_library_tags,
            commands::get_game,
            commands::get_game_detail,
            commands::check_game_version,
            commands::set_game_cover,
            commands::reset_game_cover,
            commands::update_game_user_data,
            commands::get_storage_stats,
            commands::unmatch_game,
            commands::delete_archive,
            commands::list_archives,
            commands::scan_archives,
            commands::search_f95,
            commands::resolve_f95_thread,
            commands::suggest_matches,
            commands::match_archive,
            commands::get_media_path,
            commands::download_game,
            commands::delete_platform_archive,
            commands::set_default_platform_archive,
            commands::delete_game_save,
            commands::delete_game_patch,
            commands::get_migration_status,
            commands::reorganize_archives,
            commands::assign_archive_platform,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
