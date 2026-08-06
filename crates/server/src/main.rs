use avn_hub_api::ApiState;
use avn_hub_auth::AuthService;
use avn_hub_core::AppState;
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    std::fs::create_dir_all(&config.data_dir).expect("create data dir");

    let app = AppState::new(&config.data_dir).expect("open database");

    if let Some(password) = config.bootstrap_password.as_ref() {
        if !AuthService::is_configured(&app.db).unwrap_or(false) {
            AuthService::set_password(&app.db, password).expect("set bootstrap password");
            tracing::info!("Bootstrap app password configured from environment");
        }
    }

    let api_state = ApiState {
        app: Arc::clone(&app),
        cors_origins: config.cors_origins.clone(),
    };

    let api_router = avn_hub_api::router(api_state);
    let web_router = build_web_router(&config);

    write_runtime_config(&config);

    let api_addr: SocketAddr = format!("{}:{}", config.api_host, config.api_port)
        .parse()
        .expect("api listen address");
    let web_addr: SocketAddr = format!("{}:{}", config.web_host, config.web_port)
        .parse()
        .expect("web listen address");

    tracing::info!("API listening on http://{api_addr}");
    tracing::info!("Web listening on http://{web_addr}");

    let api_listener = tokio::net::TcpListener::bind(api_addr)
        .await
        .expect("bind api");
    let web_listener = tokio::net::TcpListener::bind(web_addr)
        .await
        .expect("bind web");

    let api_server = axum::serve(api_listener, api_router);
    let web_server = axum::serve(web_listener, web_router);

    tokio::select! {
        res = api_server => {
            if let Err(e) = res {
                tracing::error!("API server error: {e}");
            }
        }
        res = web_server => {
            if let Err(e) = res {
                tracing::error!("Web server error: {e}");
            }
        }
    }
}

struct Config {
    api_host: String,
    api_port: u16,
    web_host: String,
    web_port: u16,
    data_dir: PathBuf,
    static_dir: PathBuf,
    cors_origins: Vec<String>,
    public_api_url: Option<String>,
    bootstrap_password: Option<String>,
}

impl Config {
    fn from_env() -> Self {
        let cors = std::env::var("AVN_HUB_CORS_ORIGINS").unwrap_or_default();
        let cors_origins = cors
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        let api_host = normalize_bind_host(
            std::env::var("AVN_HUB_API_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            "AVN_HUB_API_HOST",
        );
        let web_host = normalize_bind_host(
            std::env::var("AVN_HUB_WEB_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            "AVN_HUB_WEB_HOST",
        );
        let api_port = parse_port("AVN_HUB_API_PORT", 8080);
        let web_port = parse_port("AVN_HUB_WEB_PORT", 8081);

        Self {
            api_host,
            api_port,
            web_host,
            web_port,
            data_dir: PathBuf::from(
                std::env::var("AVN_HUB_DATA_DIR").unwrap_or_else(|_| "./data".into()),
            ),
            static_dir: PathBuf::from(
                std::env::var("AVN_HUB_STATIC_DIR").unwrap_or_else(|_| "./web/dist".into()),
            ),
            cors_origins,
            public_api_url: std::env::var("AVN_HUB_PUBLIC_API_URL").ok(),
            bootstrap_password: std::env::var("AVN_HUB_BOOTSTRAP_PASSWORD").ok(),
        }
    }
}

/// Bind hosts must be an IP/hostname (e.g. 0.0.0.0), never a public URL.
fn normalize_bind_host(raw: String, name: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("://") || trimmed.contains('/') {
        tracing::warn!(
            "{name}={trimmed:?} looks like a URL; bind hosts must be an address like 0.0.0.0. \
             Use AVN_HUB_PUBLIC_API_URL / CORS for public HTTPS hostnames. Falling back to 0.0.0.0."
        );
        return "0.0.0.0".into();
    }
    if trimmed.is_empty() {
        return "0.0.0.0".into();
    }
    trimmed.to_string()
}

fn parse_port(name: &str, default: u16) -> u16 {
    match std::env::var(name) {
        Ok(v) => match v.trim().parse::<u16>() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("{name}={v:?} is not a valid port; using {default}");
                default
            }
        },
        Err(_) => default,
    }
}

fn build_web_router(config: &Config) -> Router {
    let index = config.static_dir.join("index.html");
    let serve = ServeDir::new(&config.static_dir).not_found_service(ServeFile::new(index));
    Router::new().fallback_service(serve)
}

fn write_runtime_config(config: &Config) {
    let api_base = config
        .public_api_url
        .clone()
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", config.api_port));

    let payload = serde_json::json!({
        "apiBase": api_base,
    });

    let path = config.static_dir.join("config.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, payload.to_string()) {
        tracing::warn!("Could not write {}: {e}", path.display());
    } else {
        tracing::info!("Wrote runtime config to {}", path.display());
    }
}
