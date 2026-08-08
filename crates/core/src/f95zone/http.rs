//! Shared HTTP client settings for all F95Zone requests.

use crate::error::AppResult;
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::sync::Arc;
use std::time::Duration;

pub const F95_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Build a reqwest client that never sends a Referer (including on redirects).
pub fn build_client(jar: Arc<Jar>, mut headers: HeaderMap) -> AppResult<reqwest::Client> {
    if !headers.contains_key(USER_AGENT) {
        headers.insert(USER_AGENT, HeaderValue::from_static(F95_USER_AGENT));
    }

    Ok(reqwest::Client::builder()
        .cookie_provider(jar)
        .default_headers(headers)
        // Do not auto-set Referer on redirects; we also never set Referer manually.
        .referer(false)
        // Keep individual request timeouts tighter; this is a safety ceiling.
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}
