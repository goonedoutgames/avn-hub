use crate::error::{AppError, AppResult};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::sync::Arc;
use std::time::Duration;

const F95_BASE: &str = "https://f95zone.to";
const F95_LOGIN_PAGE: &str = "https://f95zone.to/login/";
const F95_LOGIN_POST: &str = "https://f95zone.to/login/login";

pub async fn login(username: &str, password: &str) -> AppResult<String> {
    let jar = Arc::new(Jar::default());
    let client = build_client(Arc::clone(&jar))?;

    let login_html = client.get(F95_LOGIN_PAGE).send().await?.text().await?;
    let token = extract_xf_token(&login_html).ok_or_else(|| {
        AppError::Other("failed to extract _xfToken from F95Zone login page".into())
    })?;

    let response = client
        .post(F95_LOGIN_POST)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(build_login_body(username, password, &token))
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if body.contains("two-step") || body.contains("two_step") {
        return Err(AppError::BadRequest(
            "F95Zone requires two-factor authentication. Use cookie login as a fallback.".into(),
        ));
    }

    if body.contains("Incorrect password") {
        return Err(AppError::BadRequest("Incorrect F95Zone username or password.".into()));
    }

    if body.contains("CAPTCHA") || body.contains("captcha") {
        return Err(AppError::BadRequest(
            "F95Zone requires CAPTCHA verification. Try again later or paste cookies manually.".into(),
        ));
    }

    // Refresh session cookies
    let _ = client.get(F95_BASE).send().await?;

    let cookies = jar_to_string(&jar)?;
    if !cookies.contains("xf_user") && !cookies.contains("xf_session") {
        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "F95Zone login failed (HTTP {status})"
            )));
        }
        return Err(AppError::BadRequest(
            "F95Zone login did not return session cookies. Check your credentials.".into(),
        ));
    }

    Ok(cookies)
}

fn build_client(jar: Arc<Jar>) -> AppResult<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    );

    Ok(reqwest::Client::builder()
        .cookie_provider(jar)
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}

fn build_login_body(username: &str, password: &str, token: &str) -> String {
    let params = [
        ("login", username),
        ("password", password),
        ("password_confirm", ""),
        ("remember", "1"),
        ("_xfRedirect", F95_BASE),
        ("_xfToken", token),
    ];
    params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn extract_xf_token(html: &str) -> Option<String> {
    for pattern in [
        r#"name="_xfToken" value=""#,
        r#"name="_xfToken" type="hidden" value=""#,
        r#"data-csrf=""#,
    ] {
        if let Some(start) = html.find(pattern) {
            let rest = &html[start + pattern.len()..];
            if let Some(end) = rest.find('"') {
                let token = rest[..end].to_string();
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }
    }
    None
}

fn jar_to_string(jar: &Jar) -> AppResult<String> {
    let header = jar
        .cookies(&reqwest::Url::parse(F95_BASE).unwrap())
        .ok_or_else(|| AppError::Other("no cookies in jar after login".into()))?;
    Ok(header.to_str().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_xf_token() {
        let html = r#"<input type="hidden" name="_xfToken" value="abc123" />"#;
        assert_eq!(extract_xf_token(html), Some("abc123".into()));
    }
}
