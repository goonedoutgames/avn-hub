use crate::error::{AppError, AppResult};
use crate::models::{AuthStatus, HttpLoginRequest, HttpLoginResponse};
use crate::state::SharedState;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
pub const SESSION_COOKIE: &str = "avn_hub_session";
const SESSION_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

pub fn session_cookie_header(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; Path=/; Max-Age={SESSION_MAX_AGE_SECS}; SameSite=Lax"
    )
}

pub fn clear_session_cookie_header() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
}

pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&format!("{SESSION_COOKIE}=")))
        .map(str::to_string)
}

pub async fn auth_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Json<AuthStatus>> {
    let settings = state.get_settings()?;
    if !settings.http_auth_configured {
        return Ok(Json(AuthStatus {
            configured: false,
            authenticated: true,
            username: None,
        }));
    }

    let authenticated = token_from_headers(&headers)
        .map(|t| state.db.session_valid(&t))
        .transpose()?
        .unwrap_or(false);

    Ok(Json(AuthStatus {
        configured: true,
        authenticated,
        username: settings.http_auth_username,
    }))
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<HttpLoginRequest>,
) -> AppResult<Response> {
    if !state.db.http_auth_configured()? {
        return Err(AppError::BadRequest(
            "HTTP authentication is not configured".into(),
        ));
    }

    if !state
        .db
        .verify_http_credentials(&req.username, &req.password)?
    {
        return Err(AppError::Unauthorized);
    }

    let token = state.db.create_session()?;
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, session_cookie_header(&token))],
        Json(HttpLoginResponse { ok: true }),
    )
        .into_response())
}

pub async fn logout(State(state): State<SharedState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = token_from_headers(&headers) {
        let _ = state.db.delete_session(&token);
    }
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, clear_session_cookie_header())],
        Json(HttpLoginResponse { ok: true }),
    )
        .into_response())
}

pub fn is_public_path(path: &str) -> bool {
    matches!(
        path,
        "/api/health" | "/api/auth/status" | "/api/auth/login"
    )
}

pub async fn require_session(state: &SharedState, headers: &HeaderMap) -> AppResult<()> {
    if !state.db.http_auth_configured()? {
        return Ok(());
    }
    let token = token_from_headers(headers).ok_or(AppError::Unauthorized)?;
    if state.db.session_valid(&token)? {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
