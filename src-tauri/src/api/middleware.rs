use crate::error::AppError;
use crate::state::SharedState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use super::auth::{is_public_path, require_session};

pub async fn auth_middleware(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();
    if is_public_path(&path) {
        return Ok(next.run(request).await);
    }

    let headers = request.headers().clone();
    if let Err(e) = require_session(&state, &headers).await {
        if matches!(e, AppError::Unauthorized) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(next.run(request).await)
}
