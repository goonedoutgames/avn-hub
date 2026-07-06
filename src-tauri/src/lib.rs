pub mod api;
mod commands;
pub mod db;
pub mod error;
pub mod models;
mod scanner;
mod sources;
pub mod state;

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
            commands::set_game_cover,
            commands::unmatch_game,
            commands::list_archives,
            commands::scan_archives,
            commands::search_f95,
            commands::suggest_matches,
            commands::match_archive,
            commands::get_media_path,
            commands::download_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
