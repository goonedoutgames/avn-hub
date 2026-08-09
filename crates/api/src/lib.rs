mod routes;

use avn_hub_auth::AuthService;
use avn_hub_core::{AppError, AppState};
use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone)]
pub struct ApiState {
    pub app: Arc<AppState>,
    pub cors_origins: Vec<String>,
}

pub fn router(state: ApiState) -> Router {
    let cors = build_cors(&state.cors_origins);
    Router::new()
        .merge(routes::router())
        .layer(cors)
        .with_state(state)
}

fn build_cors(origins: &[String]) -> CorsLayer {
    let methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ];
    // Authorization must be listed explicitly — Firefox rejects wildcard Allow-Headers for it.
    let headers = [
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
    ];

    if origins.is_empty() || origins.iter().any(|o| o == "*") {
        // Mirror the request Origin instead of sending `*`. Browsers (esp. Firefox) reject
        // `Access-Control-Allow-Origin: *` on credentialed/Authorization requests, which
        // surfaces as a generic NetworkError on cross-origin API calls (UI :8081 → API :8080).
        return CorsLayer::new()
            .allow_origin(AllowOrigin::mirror_request())
            .allow_methods(methods)
            .allow_headers(headers)
            .max_age(std::time::Duration::from_secs(600));
    }

    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(parsed))
        .allow_methods(methods)
        .allow_headers(headers)
        .max_age(std::time::Duration::from_secs(600))
}

pub struct AuthToken(pub Option<String>);

impl FromRequestParts<ApiState> for AuthToken {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let token = header.and_then(|h| {
            let h = h.trim();
            if let Some(rest) = h.strip_prefix("Bearer ") {
                Some(rest.trim().to_string())
            } else if let Some(rest) = h.strip_prefix("bearer ") {
                Some(rest.trim().to_string())
            } else {
                None
            }
        }).or_else(|| {
            parts
                .uri
                .query()
                .and_then(|q| {
                    q.split('&').find_map(|pair| {
                        let mut it = pair.splitn(2, '=');
                        let key = it.next()?;
                        let val = it.next().unwrap_or("");
                        if key == "token" && !val.is_empty() {
                            Some(val.to_string())
                        } else {
                            None
                        }
                    })
                })
        });

        Ok(AuthToken(token))
    }
}

pub struct RequireAuth;

impl FromRequestParts<ApiState> for RequireAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let AuthToken(token) = AuthToken::from_request_parts(parts, state).await?;
        AuthService::require(&state.app.db, token.as_deref()).map_err(ApiError::from)?;
        Ok(RequireAuth)
    }
}

pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.status_code())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let message = self.0.to_string();
        if status.is_server_error() {
            tracing::error!(status = %status, error = %message, "API error");
        } else {
            tracing::warn!(status = %status, error = %message, "API client error");
        }
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true, "service": "avn-hub" }))
}

pub fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
                .map(|s| s.trim().to_string())
        })
}

#[allow(dead_code)]
pub async fn ensure_auth(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<()> {
    let token = extract_token_from_headers(&headers);
    AuthService::require(&state.app.db, token.as_deref()).map_err(Into::into)
}
