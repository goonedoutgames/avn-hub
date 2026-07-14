// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "desktop")]
    avn_hub_lib::run();

    #[cfg(not(feature = "desktop"))]
    {
        eprintln!("Build with default features (desktop) for the Tauri app, or run avn-hub-server.");
        std::process::exit(1);
    }
}
