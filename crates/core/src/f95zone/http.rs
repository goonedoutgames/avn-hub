//! Shared HTTP client settings for all F95Zone requests.

use crate::error::AppResult;
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::sync::Arc;
use std::time::Duration;

pub const F95_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Build a reqwest client for F95Zone.
pub fn build_client(jar: Arc<Jar>, mut headers: HeaderMap) -> AppResult<reqwest::Client> {
    if !headers.contains_key(USER_AGENT) {
        headers.insert(USER_AGENT, HeaderValue::from_static(F95_USER_AGENT));
    }

    Ok(reqwest::Client::builder()
        .cookie_provider(jar)
        .default_headers(headers)
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()?)
}
