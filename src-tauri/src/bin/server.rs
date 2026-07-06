use avn_hub_lib::api::run_server;
use avn_hub_lib::db::default_data_dir;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "avn_hub=info,tower_http=info".into()),
        )
        .init();

    let host = env::var("AVN_HUB_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = env::var("AVN_HUB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let data_dir = env::var("AVN_HUB_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_data_dir());
    let static_dir = env::var("AVN_HUB_STATIC_DIR").ok().map(PathBuf::from);

    if let Err(e) = run_server(&host, port, data_dir, static_dir).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
