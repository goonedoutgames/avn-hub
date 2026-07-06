pub mod auth;
pub mod text;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::F95SearchResult;
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const F95_LATEST_DATA_URL: &str = "https://f95zone.to/sam/latest_alpha/latest_data.php";
const F95_BASE_URL: &str = "https://f95zone.to";

/// Extract a F95Zone thread id from a URL, path, or bare numeric id.
pub fn parse_f95_thread_id(input: &str) -> Option<i64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return s.parse().ok();
    }

    let lower = s.to_lowercase();
    let needle = "/threads/";
    let idx = lower.find(needle)?;
    let rest = &s[idx + needle.len()..];
    let path = rest.split(['?', '#']).next()?.trim_end_matches('/');

    if let Some((_, id_part)) = path.rsplit_once('.') {
        id_part.parse().ok()
    } else {
        path.parse().ok()
    }
}

pub struct F95Client {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct F95ListResponse {
    status: String,
    msg: Option<F95ListMessage>,
}

#[derive(Debug, Deserialize)]
struct F95ListMessage {
    data: Vec<F95Item>,
}

#[derive(Debug, Deserialize)]
struct F95Item {
    thread_id: i64,
    title: String,
    #[serde(default, deserialize_with = "de::opt_string")]
    creator: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    version: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    cover: Option<String>,
    #[serde(default, deserialize_with = "de::vec_string")]
    screens: Vec<String>,
    #[serde(default, deserialize_with = "de::opt_vec_string")]
    tags: Option<Vec<String>>,
    #[serde(default)]
    rating: Option<f64>,
    #[serde(default, deserialize_with = "de::opt_string")]
    date: Option<String>,
}

mod de {
    use serde::{Deserialize, Deserializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flexible {
        Str(String),
        Int(i64),
        Float(f64),
        Bool(bool),
    }

    impl Flexible {
        fn into_string(self) -> String {
            match self {
                Flexible::Str(s) => s,
                Flexible::Int(n) => n.to_string(),
                Flexible::Float(f) => {
                    if f.fract() == 0.0 {
                        format!("{f:.0}")
                    } else {
                        f.to_string()
                    }
                }
                Flexible::Bool(b) => b.to_string(),
            }
        }
    }

    pub fn opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<Flexible>::deserialize(deserializer)?;
        Ok(value.map(Flexible::into_string))
    }

    pub fn vec_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Option::<Vec<Flexible>>::deserialize(deserializer)?;
        Ok(values
            .unwrap_or_default()
            .into_iter()
            .map(Flexible::into_string)
            .collect())
    }

    pub fn opt_vec_string<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Option::<Vec<Flexible>>::deserialize(deserializer)?;
        Ok(values.map(|v| v.into_iter().map(Flexible::into_string).collect()))
    }
}

#[derive(Debug, Clone)]
pub struct ThreadMetadata {
    pub result: F95SearchResult,
    pub screenshots: Vec<String>,
    pub all_images: Vec<String>,
    pub description: Option<String>,
}

impl F95Client {
    pub fn from_cookies(cookies: &str) -> AppResult<Self> {
        let jar = Arc::new(Jar::default());
        let url = reqwest::Url::parse(F95_BASE_URL).unwrap();
        jar.add_cookie_str(cookies, &url);

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Requested-With",
            HeaderValue::from_static("XMLHttpRequest"),
        );
        headers.insert(
            "Accept",
            HeaderValue::from_static("application/json, text/javascript, */*; q=0.01"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            ),
        );
        if let Ok(val) = HeaderValue::from_str(cookies) {
            headers.insert(COOKIE, val);
        }

        let client = reqwest::Client::builder()
            .cookie_provider(jar)
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self { client })
    }

    pub async fn probe_auth(&self) -> AppResult<bool> {
        let url = format!("{F95_LATEST_DATA_URL}?cmd=list&cat=games&page=1&rows=1");
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Ok(false);
        }
        let text = response.text().await?;
        Ok(parse_list_response(&text).is_ok())
    }

    pub async fn search(&self, query: &str, page: u32) -> AppResult<Vec<F95SearchResult>> {
        let query = text::normalize_apostrophes(query.trim());
        let url = format!(
            "{F95_LATEST_DATA_URL}?cmd=list&cat=games&sort=date&page={page}&rows=30&search={}",
            urlencoding::encode(&query)
        );

        let response = self.client.get(&url).send().await?;
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(AppError::BadRequest(
                "F95Zone returned 403. Check credentials in Settings.".into(),
            ));
        }
        if !response.status().is_success() {
            return Err(AppError::Other(format!(
                "F95Zone request failed: {}",
                response.status()
            )));
        }

        let text = response.text().await?;
        parse_list_response(&text)
    }

    pub async fn fetch_list_entry(&self, thread_id: i64) -> AppResult<Option<F95SearchResult>> {
        let results = self.search(&thread_id.to_string(), 1).await?;
        Ok(results.into_iter().find(|r| r.thread_id == thread_id))
    }

    pub async fn fetch_thread_metadata(&self, thread_id: i64) -> AppResult<ThreadMetadata> {
        let url = format!("{F95_BASE_URL}/threads/{thread_id}/");
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(AppError::Other(format!(
                "failed to fetch thread {thread_id}: {}",
                response.status()
            )));
        }
        let html = response.text().await?;
        parse_thread_html(thread_id, &html)
    }
}

fn parse_list_response(text: &str) -> AppResult<Vec<F95SearchResult>> {
    let trimmed = text.trim();
    if trimmed.starts_with('<') || trimmed.starts_with("<!") {
        return Err(AppError::BadRequest(
            "F95Zone returned HTML instead of JSON. Log in via Settings (credentials or cookies)."
                .into(),
        ));
    }

    let body: F95ListResponse = serde_json::from_str(trimmed).map_err(|e| {
        let preview: String = trimmed.chars().take(120).collect();
        AppError::Other(format!(
            "failed to parse F95Zone response: {e}. Preview: {preview}"
        ))
    })?;

    if body.status != "ok" {
        return Err(AppError::Other(format!(
            "F95Zone returned status: {}",
            body.status
        )));
    }

    let items = body.msg.map(|m| m.data).unwrap_or_default();
    Ok(items.into_iter().map(item_to_result).collect())
}

fn item_to_result(item: F95Item) -> F95SearchResult {
    let mut screenshots: Vec<String> = item
        .screens
        .into_iter()
        .filter_map(|s| text::sam_list_media_url(&s))
        .collect();
    let mut cover = item
        .cover
        .as_deref()
        .and_then(text::sam_list_media_url)
        .unwrap_or_default();

    if cover.is_empty() && !screenshots.is_empty() {
        cover = text::pick_best_cover("", &screenshots);
    } else if !cover.is_empty() {
        cover = text::pick_best_cover(&cover, &screenshots);
    }

    if screenshots.is_empty() && !cover.is_empty() {
        screenshots.push(cover.clone());
    }

    F95SearchResult {
        thread_id: item.thread_id,
        title: text::clean_f95_title(&item.title),
        creator: text::decode_html_entities(
            &item.creator.unwrap_or_else(|| "Unknown".into()),
        ),
        version: item
            .version
            .map(|v| text::decode_html_entities(&v))
            .filter(|v| !v.is_empty() && v != "Unknown")
            .unwrap_or_default(),
        cover,
        screenshots,
        tags: item.tags.unwrap_or_default(),
        rating: item.rating.unwrap_or(0.0),
        url: format!("{F95_BASE_URL}/threads/{}/", item.thread_id),
        date: item.date.unwrap_or_default(),
    }
}

fn parse_thread_html(thread_id: i64, html: &str) -> AppResult<ThreadMetadata> {
    let raw_title = extract_thread_title(html)
        .or_else(|| extract_meta_content(html, "og:title"))
        .or_else(|| extract_tag_text(html, "h1"))
        .unwrap_or_else(|| format!("Thread {thread_id}"));

    let title = text::clean_f95_title(&raw_title);

    let og_cover = extract_meta_content(html, "og:image").unwrap_or_default();
    let description = extract_first_post_description(html).or_else(|| {
        extract_meta_content(html, "og:description")
            .map(|d| text::decode_html_entities(&d))
            .filter(|d| !d.ends_with("..."))
    });
    let post_images = extract_first_post_images(html);
    let (cover, screenshots) = text::split_cover_and_screenshots(&post_images);
    let cover = if cover.is_empty() {
        text::pick_best_cover(&og_cover, &post_images)
    } else {
        cover
    };

    Ok(ThreadMetadata {
        result: F95SearchResult {
            thread_id,
            title,
            creator: extract_creator(html),
            version: extract_version(html),
            cover: cover.clone(),
            screenshots: screenshots.clone(),
            tags: extract_tags(html),
            rating: 0.0,
            url: format!("{F95_BASE_URL}/threads/{thread_id}/"),
            date: String::new(),
        },
        screenshots,
        all_images: post_images,
        description,
    })
}

fn extract_thread_title(html: &str) -> Option<String> {
    for marker in ["p-title-value", "thread-title"] {
        if let Some(idx) = html.find(marker) {
            let end = (idx + 500).min(html.len());
            let slice = &html[idx..end];
            if let Some(start) = slice.find('>') {
                if let Some(end) = slice[start..].find('<') {
                    let t = slice[start + 1..start + end].trim();
                    if !t.is_empty() {
                        return Some(text::decode_html_entities(t));
                    }
                }
            }
        }
    }
    None
}

fn extract_first_post_description(html: &str) -> Option<String> {
    let body = extract_bb_wrapper(html)?;
    extract_overview_section(&body).or_else(|| extract_post_body_text(&body))
}

/// Full first-post text trimmed before changelog / release metadata.
fn extract_post_body_text(bb_html: &str) -> Option<String> {
    let mut text = html_fragment_to_text(bb_html);
    text = text::decode_html_entities(&text);
    text = text.replace('\u{200b}', "");
    text = strip_spoiler_noise(&text);
    if let Some(end) = find_thread_updated_marker(&text) {
        text = text[..end].trim().to_string();
    }
    text = normalize_description_text(&text);
    if text.len() < 40 {
        None
    } else {
        Some(text)
    }
}

fn extract_overview_section(bb_html: &str) -> Option<String> {
    let mut text = html_fragment_to_text(bb_html);
    text = text::decode_html_entities(&text);
    text = text.replace('\u{200b}', "");
    text = strip_spoiler_noise(&text);

    let lower = text.to_lowercase();
    let start = lower
        .find("overview:")
        .or_else(|| lower.find("**overview:**"))?;
    let from_overview = &text[start..];
    let end = find_thread_updated_marker(from_overview).unwrap_or(from_overview.len());
    let mut slice = from_overview[..end].trim().to_string();

    let slice_lower = slice.to_lowercase();
    if slice_lower.starts_with("overview:") {
        slice = slice[9..].trim().to_string();
    } else if slice_lower.starts_with("**overview:**") {
        slice = slice[13..].trim().to_string();
    }

    slice = normalize_description_text(&slice);
    if slice.is_empty() {
        None
    } else {
        Some(slice)
    }
}

fn strip_spoiler_noise(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim().to_lowercase();
            !t.is_empty() && t != "spoiler" && !t.starts_with("spoiler:")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_thread_updated_marker(text: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    for marker in ["thread updated:", "thread update:"] {
        if let Some(idx) = lower.find(marker) {
            return Some(idx);
        }
    }
    None
}

fn html_fragment_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for pat in [
        "<br>", "<br/>", "<br />", "<BR>", "<BR/>", "<BR />", "</p>", "</div>", "</li>",
        "</P>", "</DIV>", "</LI>",
    ] {
        s = s.replace(pat, "\n");
    }

    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn normalize_description_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut result = String::new();
    let mut prev_blank = false;
    for line in lines {
        if line.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push('\n');
                prev_blank = true;
            }
        } else {
            if !result.is_empty() && !prev_blank {
                result.push('\n');
            }
            result.push_str(line);
            prev_blank = false;
        }
    }
    result.trim().to_string()
}

fn extract_first_post_images(html: &str) -> Vec<String> {
    let search_html = extract_bb_wrapper(html).unwrap_or_default();
    if search_html.is_empty() {
        return Vec::new();
    }

    let mut images = Vec::new();

    for fragment in search_html.split("js-lbImage") {
        if let Some(url) = extract_best_attachment_url(fragment) {
            push_image_url(&mut images, &url);
        }
    }

    scan_attachment_cdn_urls(&search_html, &mut images);

    for fragment in search_html.split("<img") {
        if let Some(url) = extract_attr(fragment, "data-url")
            .or_else(|| extract_attr(fragment, "data-src"))
        {
            push_image_url(&mut images, &url);
        }
    }

    for fragment in search_html.split("<a") {
        if let Some(url) = extract_attr(fragment, "href") {
            if url.contains("attachments") {
                push_image_url(&mut images, &url);
            }
        }
    }

    resolve_inline_attachment_filenames(&search_html, &mut images);
    scan_attachment_link_urls(&search_html, &mut images);

    dedupe_upgraded_images(images)
}

/// XenForo attachment page links — resolved to CDN during download.
fn scan_attachment_link_urls(html: &str, images: &mut Vec<String>) {
    for fragment in html.split("href=\"") {
        let Some(rest) = fragment.split_once('"') else {
            continue;
        };
        let url = normalize_url(rest.0);
        if url.contains("/attachments/") && !text::is_xenforo_thumbnail(&url) {
            if !images.iter().any(|u| u == &url) {
                images.push(url);
            }
        }
    }
    for fragment in html.split("href='") {
        let Some(rest) = fragment.split_once('\'') else {
            continue;
        };
        let url = normalize_url(rest.0);
        if url.contains("/attachments/") && !text::is_xenforo_thumbnail(&url) {
            if !images.iter().any(|u| u == &url) {
                images.push(url);
            }
        }
    }
}

fn extract_best_attachment_url(fragment: &str) -> Option<String> {
    for attr in ["data-url", "data-src", "href"] {
        if let Some(url) = extract_attr(fragment, attr) {
            let normalized = normalize_url(&url);
            if normalized.contains("attachments.f95zone.to") {
                return Some(text::upgrade_image_url(&normalized));
            }
            if normalized.contains("/attachments/") && !text::is_xenforo_thumbnail(&normalized) {
                return Some(normalized);
            }
        }
    }
    None
}

fn scan_attachment_cdn_urls(html: &str, images: &mut Vec<String>) {
    for prefix in [
        "https://attachments.f95zone.to/",
        "http://attachments.f95zone.to/",
        "//attachments.f95zone.to/",
    ] {
        let mut pos = 0usize;
        while let Some(rel) = html[pos..].find(prefix) {
            let start = pos + rel;
            let url_start = html[..start]
                .rfind(|c: char| c == '"' || c == '\'' || c == '(' || c == ' ')
                .map(|i| i + 1)
                .unwrap_or(start);
            let slice = &html[url_start..];
            let end = slice
                .find(|c: char| c == '"' || c == '\'' || c == '<' || c == ' ' || c == ')')
                .unwrap_or(slice.len());
            push_image_url(images, &slice[..end]);
            pos = url_start + end;
        }
    }
}

/// Match bare filenames (e.g. c1s0r19.png) to full CDN URLs already present in the HTML.
fn resolve_inline_attachment_filenames(html: &str, images: &mut Vec<String>) {
    let known: Vec<String> = images
        .iter()
        .map(|u| text::upgrade_image_url(u))
        .collect();

    for token in html.split_whitespace() {
        let token = token
            .trim_matches(|c: char| c == '"' || c == '\'' || c == '>' || c == '<' || c == ',');
        if !is_image_filename(token) {
            continue;
        }
        if let Some(url) = known.iter().find(|u| u.to_lowercase().ends_with(&token.to_lowercase())) {
            push_image_url(images, url);
        }
    }
}

fn is_image_filename(token: &str) -> bool {
    let lower = token.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
}

fn push_image_url(images: &mut Vec<String>, url: &str) {
    if !is_image_url(url) {
        return;
    }
    let url = text::upgrade_image_url(&normalize_url(url));
    if url.is_empty()
        || text::is_branding_image(&url)
        || text::is_xenforo_thumbnail(&url)
        || url.contains("avatar")
        || url.contains("smilie")
        || url.contains("/styles/")
    {
        return;
    }
    images.push(url);
}

fn dedupe_upgraded_images(images: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for url in images {
        let key = url.to_lowercase();
        if seen.insert(key) {
            out.push(url);
        }
    }
    out.truncate(40);
    out
}

fn extract_bb_wrapper(html: &str) -> Option<String> {
    let post = extract_thread_starter_post(html)?;
    let marker = "bbWrapper";
    let idx = post.find(marker)?;
    let after = &post[idx..];
    let content_start = after.find('>')? + 1;
    let inner = &after[content_start..];

    let mut depth = 1i32;
    let mut pos = 0usize;
    while pos < inner.len() {
        let rel_open = inner[pos..].find("<div");
        let rel_close = inner[pos..].find("</div>");

        let next_close = rel_close.map(|r| (r, true));
        let next_open = rel_open
            .filter(|&r| is_tag_boundary(inner, pos + r + 4))
            .map(|r| (r, false));

        let next = match (next_open, next_close) {
            (Some(o), Some(c)) => Some(if o.0 <= c.0 { o } else { c }),
            (Some(o), None) => Some(o),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        };

        let Some((rel, is_close)) = next else {
            break;
        };

        let at = pos + rel;
        if is_close {
            depth -= 1;
            if depth == 0 {
                return Some(inner[..at].to_string());
            }
            pos = at + 6;
        } else {
            depth += 1;
            pos = at + 4;
        }
    }
    None
}

fn is_tag_boundary(s: &str, idx: usize) -> bool {
    s.as_bytes()
        .get(idx)
        .is_none_or(|&b| !b.is_ascii_alphanumeric())
}

fn extract_thread_starter_post(html: &str) -> Option<String> {
    let markers = ["message-threadStarterPost", "threadStarterPost"];
    for marker in markers {
        if let Some(idx) = html.find(marker) {
            let article_start = html[..idx].rfind("<article")?;
            let article_html = &html[article_start..];
            let article_end = article_html.find("</article>")? + "</article>".len();
            return Some(article_html[..article_end].to_string());
        }
    }

    // Fallback: first message-body block.
    let idx = html.find("class=\"message-body")?;
    let end = (idx + 80_000).min(html.len());
    Some(html[idx..end].to_string())
}

#[allow(dead_code)]
fn extract_first_message_body(html: &str) -> Option<String> {
    extract_bb_wrapper(html)
}

fn extract_attr(fragment: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let pattern2 = format!("{attr}='");
    if let Some(start) = fragment.find(&pattern) {
        let rest = &fragment[start + pattern.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    if let Some(start) = fragment.find(&pattern2) {
        let rest = &fragment[start + pattern2.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn is_image_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("attachments.f95zone")
        || lower.contains("/attachments/")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
}

fn normalize_url(url: &str) -> String {
    let decoded = url.replace("&amp;", "&");
    if decoded.starts_with("//") {
        format!("https:{decoded}")
    } else if decoded.starts_with('/') {
        format!("{F95_BASE_URL}{decoded}")
    } else {
        decoded
    }
}

fn extract_meta_content(html: &str, property: &str) -> Option<String> {
    for pattern in [
        format!(r#"property="{property}" content=""#),
        format!(r#"name="{property}" content=""#),
    ] {
        if let Some(start) = html.find(&pattern) {
            let rest = &html[start + pattern.len()..];
            if let Some(end) = rest.find('"') {
                return Some(rest[..end].replace("&amp;", "&"));
            }
        }
    }
    None
}

fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = html.find(&open)?;
    let content_start = html[start..].find('>')? + start + 1;
    let rest = &html[content_start..];
    let end = rest.find(&format!("</{tag}>"))?;
    let text = rest[..end].trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn extract_creator(html: &str) -> String {
    for marker in ["Developer", "dev", "creator"] {
        if let Some(idx) = html.to_lowercase().find(&marker.to_lowercase()) {
            let end = (idx + 200).min(html.len());
            let snippet = &html[idx..end];
            if let Some(link_start) = snippet.find("<a ") {
                if let Some(href_end) = snippet[link_start..].find('>') {
                    let after = &snippet[link_start + href_end + 1..];
                    if let Some(close) = after.find("</a>") {
                        let name = after[..close].trim().replace("&amp;", "&");
                        if !name.is_empty() {
                            return name;
                        }
                    }
                }
            }
        }
    }
    "Unknown".into()
}

fn extract_version(html: &str) -> String {
    for marker in ["Version", "version"] {
        if let Some(idx) = html.find(marker) {
            let end = (idx + 100).min(html.len());
            let snippet = &html[idx..end];
            for part in snippet.split(|c: char| c == '<' || c == '>') {
                let trimmed = part.trim();
                if trimmed.chars().any(|c| c.is_ascii_digit()) && trimmed.len() < 30 {
                    return trimmed.to_string();
                }
            }
        }
    }
    String::new()
}

fn extract_tags(html: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for part in html.split("tagItem") {
        if let Some(start) = part.find('>') {
            if let Some(end) = part[start..].find('<') {
                let tag = part[start + 1..start + end].trim();
                if !tag.is_empty() && tag.len() < 50 {
                    tags.push(tag.to_string());
                }
            }
        }
    }
    tags.truncate(20);
    tags
}

pub async fn cache_thread_media(
    db: &Database,
    client: &F95Client,
    game_id: i64,
    thread_id: i64,
    cover_url: &str,
    screenshots: &[String],
) -> AppResult<Option<String>> {
    let media_dir = db.media_dir().join(format!("{thread_id}"));
    if media_dir.exists() {
        let _ = std::fs::remove_dir_all(&media_dir);
    }
    std::fs::create_dir_all(&media_dir)?;
    db.clear_game_media(game_id)?;

    let upgraded_screenshots: Vec<String> = screenshots
        .iter()
        .filter_map(|s| text::download_media_url(s).or_else(|| text::sam_list_media_url(s)))
        .collect();
    let cover_candidate = text::download_media_url(cover_url)
        .or_else(|| text::sam_list_media_url(cover_url))
        .unwrap_or_default();
    let effective_cover = text::pick_best_cover(&cover_candidate, &upgraded_screenshots);
    let mut cover_path = None;
    let mut stored_cover_url = String::new();

    if !effective_cover.is_empty() {
        if let Some((path, resolved)) =
            download_image(client, &effective_cover, &media_dir, "cover").await?
        {
            stored_cover_url = resolved;
            db.insert_media(game_id, &stored_cover_url, &path, "cover")?;
            cover_path = Some(path);
        }
    }

    let mut ss_index = 0;
    for url in upgraded_screenshots
        .iter()
        .filter(|u| !u.is_empty() && !text::is_branding_image(u))
    {
        if ss_index >= 30 {
            break;
        }
        if !stored_cover_url.is_empty()
            && text::upgrade_image_url(url) == text::upgrade_image_url(&stored_cover_url)
        {
            continue;
        }
        if let Some((path, resolved)) =
            download_image(client, url, &media_dir, &format!("ss_{ss_index}")).await?
        {
            db.insert_media(game_id, &resolved, &path, "screenshot")?;
            ss_index += 1;
        }
    }

    // Use first screenshot as cover if cover download failed
    if cover_path.is_none() && ss_index > 0 {
        if let Ok(media) = db.list_game_media(game_id) {
            if let Some(path) = media
                .into_iter()
                .find(|m| m.media_type == "screenshot")
                .and_then(|m| m.local_path)
            {
                cover_path = Some(path);
            }
        }
    }

    Ok(cover_path)
}

async fn download_image(
    client: &F95Client,
    url: &str,
    dir: &Path,
    basename: &str,
) -> AppResult<Option<(String, String)>> {
    if url.is_empty() {
        return Ok(None);
    }

    let resolved = resolve_download_url(client, url).await;

    let response = client
        .client
        .get(&resolved)
        .header("Referer", "https://f95zone.to/")
        .send()
        .await?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let ext = if content_type.contains("avif") {
        "avif"
    } else if content_type.contains("webp") {
        "webp"
    } else if content_type.contains("jpeg") || content_type.contains("jpg") {
        "jpg"
    } else if content_type.contains("png") {
        "png"
    } else if content_type.contains("gif") {
        "gif"
    } else {
        resolved
            .rsplit('.')
            .next()
            .and_then(|e| e.split('?').next())
            .filter(|e| {
                matches!(
                    e.to_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif"
                )
            })
            .unwrap_or("jpg")
    };

    let path = dir.join(format!("{basename}.{ext}"));
    let final_url = response.url().to_string();
    let bytes = response.bytes().await?;
    std::fs::write(&path, &bytes)?;
    Ok(Some((
        path.display().to_string(),
        text::upgrade_image_url(&final_url),
    )))
}

async fn resolve_download_url(client: &F95Client, url: &str) -> String {
    let original = text::upgrade_image_url(url);
    if text::is_cdn_attachment(&original) {
        return original;
    }

    let page_url = text::attachment_page_url(url);
    let fetch_targets: Vec<String> = if page_url.contains("/attachments/") {
        let mut targets = vec![page_url];
        if original.contains("/thumbnail") && original != text::attachment_page_url(url) {
            targets.insert(0, original.clone());
        }
        targets
    } else if original.contains("/attachments/") {
        vec![original.clone()]
    } else {
        return original;
    };

    for target in fetch_targets {
        if let Ok(response) = client.client.get(&target).send().await {
            let final_url = response.url().to_string();
            if text::is_cdn_attachment(&final_url) {
                return text::upgrade_image_url(&final_url);
            }

            if let Ok(body) = response.text().await {
                for fragment in body.split("data-url=\"") {
                    let Some((rest, _)) = fragment.split_once('"') else {
                        continue;
                    };
                    let u = normalize_url(rest);
                    if text::is_cdn_attachment(&u) {
                        return text::upgrade_image_url(&u);
                    }
                }
                for fragment in body.split("data-url='") {
                    let Some((rest, _)) = fragment.split_once('\'') else {
                        continue;
                    };
                    let u = normalize_url(rest);
                    if text::is_cdn_attachment(&u) {
                        return text::upgrade_image_url(&u);
                    }
                }
                if let Some(cdn) = first_cdn_url_in_html(&body) {
                    return cdn;
                }
                if let Some(og) = extract_meta_content(&body, "og:image") {
                    let og = text::upgrade_image_url(&og);
                    if text::is_cdn_attachment(&og) {
                        return og;
                    }
                }
            }
        }
    }

    original
}

fn first_cdn_url_in_html(html: &str) -> Option<String> {
    for prefix in [
        "https://attachments.f95zone.to/",
        "http://attachments.f95zone.to/",
        "//attachments.f95zone.to/",
    ] {
        if let Some(idx) = html.find(prefix) {
            let slice = &html[idx..];
            let end = slice
                .find(|c: char| c == '"' || c == '\'' || c == '<' || c == ' ' || c == ')')
                .unwrap_or(slice.len());
            let url = text::upgrade_image_url(&slice[..end]);
            if text::is_cdn_attachment(&url) {
                return Some(url);
            }
        }
    }
    None
}

pub fn media_url_to_api_path(local_path: &str, data_dir: &Path) -> Option<String> {
    let path = Path::new(local_path);
    let media_root = data_dir.join("media");
    if !path.starts_with(&media_root) {
        return None;
    }
    let relative = path.strip_prefix(&media_root).ok()?;
    Some(format!("/api/media/{}", relative.display()))
}

// Backwards compat alias
pub fn cover_url_to_api_path(cover_path: &str, data_dir: &Path) -> Option<String> {
    media_url_to_api_path(cover_path, data_dir)
}

#[cfg(test)]
mod download_tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn downloads_attachment_cdn_banner() {
        let client = F95Client::from_cookies("").expect("client");
        let dir = std::env::temp_dir().join("avn_hub_download_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let result = download_image(
            &client,
            "https://attachments.f95zone.to/2026/05/6066253_Chapter4Banner.png",
            Path::new(&dir),
            "cover",
        )
        .await
        .expect("download");
        let (path, url) = result.expect("should download");
        assert!(url.contains("attachments.f95zone.to"));
        assert!(std::path::Path::new(&path).exists());
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 50_000, "expected full-res file, got {} bytes", meta.len());
    }
}

#[cfg(test)]
mod description_extraction_tests {
    use super::{extract_bb_wrapper, extract_overview_section, extract_thread_starter_post};

    #[test]
    fn thread_starter_post_from_real_html() {
        let path = std::path::Path::new("/tmp/f95_262861.html");
        if !path.exists() {
            return;
        }
        let html = std::fs::read_to_string(path).expect("read html");
        let post = extract_thread_starter_post(&html).expect("starter post");
        assert!(post.contains("bbWrapper"), "post missing bbWrapper");
        assert!(post.len() > 5000, "post too short: {}", post.len());
    }

    #[test]
    fn bb_wrapper_extracts_from_real_thread_html() {
        let path = std::path::Path::new("/tmp/f95_262861.html");
        if !path.exists() {
            return;
        }
        let html = std::fs::read_to_string(path).expect("read html");
        let body = extract_bb_wrapper(&html).expect("bbWrapper");
        assert!(body.len() > 1000, "bbWrapper too short: {}", body.len());
        let section = extract_overview_section(&body).expect("overview section");
        assert!(section.len() > 500, "overview too short: {}", section.len());
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_f95_thread_id, parse_list_response, parse_thread_html};

    #[test]
    fn parses_f95_thread_urls() {
        assert_eq!(
            parse_f95_thread_id(
                "https://f95zone.to/threads/angels-love-v0-4-pe-gpoint.267605/"
            ),
            Some(267605)
        );
        assert_eq!(
            parse_f95_thread_id(
                "https://f95zone.to/threads/arianas-perverted-diary-v0-7-3-girls-on-top.161606/"
            ),
            Some(161606)
        );
        assert_eq!(parse_f95_thread_id("267605"), Some(267605));
        assert_eq!(parse_f95_thread_id("/threads/161606/"), Some(161606));
        assert_eq!(parse_f95_thread_id("not a url"), None);
    }

    #[test]
    fn parses_numeric_version() {
        let json = r#"{"status":"ok","msg":{"data":[{"thread_id":262861,"title":"Actual Roommates 2","creator":"HanakoXVN","version":107}]}}"#;
        let results = parse_list_response(json).unwrap();
        assert_eq!(results[0].version, "107");
    }

    #[test]
    fn fills_cover_from_screens_when_sam_cover_empty() {
        let json = r#"{"status":"ok","msg":{"data":[{"thread_id":262861,"title":"Actual Roommates 2","creator":"HanakoXVN","version":"Ch.4","screens":["https://preview.f95zone.to/2025/07/5083149_c1s16tr2.png","https://preview.f95zone.to/2026/05/6066253_Chapter4Banner.png"]}]}}"#;
        let results = parse_list_response(json).unwrap();
        assert!(results[0].cover.contains("Chapter4Banner"));
    }

    #[test]
    fn preserves_sam_cover_urls_for_display() {
        let json = r#"{"status":"ok","msg":{"data":[{"thread_id":1,"title":"Test","cover":"https://f95zone.to/attachments/foo.12345/thumbnail","screens":[]}]}}"#;
        let results = parse_list_response(json).unwrap();
        assert!(results[0].cover.contains("attachments"));
    }

    #[test]
    fn extracts_description_until_thread_updated() {
        let html = r#"
        <article class="message message-threadStarterPost">
          <div class="message-body">
            <div class="bbWrapper">
              <img src="https://attachments.f95zone.to/2026/05/6066253_Chapter4Banner.png" />
              <p><b>Overview:</b></p>
              <p>Welcome to Blairmont University!</p>
              <p>Play as the daughter of Lawrence.</p>
              <div><b>Thread Updated</b>: 2026-05-16</div>
              <p>Release Date: 2026-05-16</p>
            </div>
          </div>
        </article>
        "#;
        let meta = parse_thread_html(1, html).unwrap();
        let desc = meta.description.unwrap();
        assert!(desc.contains("Welcome to Blairmont University"));
        assert!(desc.contains("Play as the daughter"));
        assert!(!desc.contains("Release Date"));
        assert!(!desc.contains("Overview:"));
        assert!(meta.result.cover.contains("Chapter4Banner"));
    }

    #[test]
    fn extracts_full_description_from_real_thread_html() {
        let path = std::path::Path::new("/tmp/f95_262861.html");
        if !path.exists() {
            return;
        }
        let html = std::fs::read_to_string(path).expect("read html");
        let meta = parse_thread_html(262861, &html).unwrap();
        let desc = meta.description.expect("description");
        assert!(
            desc.len() > 500,
            "expected full overview text, got {} chars ending with {:?}",
            desc.len(),
            desc.chars().rev().take(40).collect::<String>()
        );
        assert!(desc.contains("Legendary"));
        assert!(!desc.ends_with("..."));
        assert!(!desc.contains("Release Date"));
    }

    #[test]
    fn collects_inline_attachment_filenames() {
        let html = r#"
        <article class="message message-threadStarterPost">
          <div class="bbWrapper">
            <a href="https://attachments.f95zone.to/2025/07/5083134_c1s0r19.png">img</a>
            c1s0r19.png c1s4r3.png
          </div>
        </article>
        "#;
        let meta = parse_thread_html(1, html).unwrap();
        assert!(meta
            .all_images
            .iter()
            .any(|u| u.contains("5083134_c1s0r19")));
    }
}
