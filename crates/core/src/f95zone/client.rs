// auth is sibling module
// text is sibling module

use super::http;
use super::tags::{self, TagCatalog};
use super::text;
use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{DownloadLink, F95SearchResult};
use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const F95_LATEST_DATA_URL: &str = "https://f95zone.to/sam/latest_alpha/latest_data.php";
const F95_BASE_URL: &str = "https://f95zone.to";

/// Floor `i` to the nearest char boundary at or before it.
fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Ceil `i` to the nearest char boundary at or after it.
fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Slice `s[start..end]` without panicking on mid-UTF-8 indexes.
/// Panics here abort the request with no HTTP body → Cloudflare 502.
fn safe_slice(s: &str, start: usize, end: usize) -> &str {
    let start = floor_char_boundary(s, start.min(s.len()));
    let end = ceil_char_boundary(s, end.min(s.len())).max(start);
    &s[start..end]
}

fn panic_payload_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".into()
    }
}

/// Extract a F95Zone thread id from a URL, path, or bare numeric id.
pub fn parse_f95_thread_id(input: &str) -> Option<i64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return s.parse().ok();
    }

    let path = f95_thread_path(s)?;
    if let Some((_, id_part)) = path.rsplit_once('.') {
        id_part.parse().ok()
    } else {
        path.parse().ok()
    }
}

/// Slug segment from `/threads/{slug}.{id}/` (before the numeric id), if present.
pub fn parse_f95_thread_slug(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() || s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let path = f95_thread_path(s)?;
    let (slug, id_part) = path.rsplit_once('.')?;
    if id_part.chars().all(|c| c.is_ascii_digit()) && !slug.is_empty() {
        Some(slug.to_string())
    } else {
        None
    }
}

fn f95_thread_path(s: &str) -> Option<&str> {
    // ASCII lowercase keeps byte indexes aligned with `s` (Unicode to_lowercase does not).
    let lower = s.to_ascii_lowercase();
    let needle = "/threads/";
    let idx = lower.find(needle)?;
    let rest = &s[idx + needle.len()..];
    Some(rest.split(['?', '#']).next()?.trim_end_matches('/'))
}

#[derive(Clone)]
pub struct F95Client {
    pub(crate) client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct F95ListResponse {
    status: String,
    msg: Option<F95ListMessage>,
}

#[derive(Debug, Deserialize)]
struct F95ListMessage {
    data: Vec<F95Item>,
    #[serde(default)]
    pagination: Option<F95Pagination>,
}

/// SAM list pagination (`msg.pagination`).
#[derive(Debug, Deserialize)]
struct F95Pagination {
    #[serde(default)]
    page: u32,
    /// Total number of pages for the current filter set.
    #[serde(default)]
    total: u32,
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
    #[serde(default, deserialize_with = "de::opt_vec_string")]
    prefixes: Option<Vec<String>>,
    #[serde(default, deserialize_with = "de::opt_f64")]
    rating: Option<f64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    likes: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    views: Option<i64>,
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

    pub fn opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<Flexible>::deserialize(deserializer)?;
        Ok(value.and_then(|v| match v {
            Flexible::Int(n) => Some(n),
            Flexible::Float(f) => Some(f as i64),
            Flexible::Str(s) => s.replace(',', "").parse().ok(),
            Flexible::Bool(_) => None,
        }))
    }

    pub fn opt_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<Flexible>::deserialize(deserializer)?;
        Ok(value.and_then(|v| match v {
            Flexible::Float(f) => Some(f),
            Flexible::Int(n) => Some(n as f64),
            Flexible::Str(s) => s.trim().replace(',', "").parse().ok(),
            Flexible::Bool(_) => None,
        }))
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
        if let Ok(val) = HeaderValue::from_str(cookies) {
            headers.insert(COOKIE, val);
        }

        let client = http::build_client(jar, headers)?;
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

    pub async fn search(
        &self,
        query: &str,
        page: u32,
        sort: &str,
    ) -> AppResult<Vec<F95SearchResult>> {
        Ok(self
            .search_filtered(CatalogFilter {
                search: query.to_string(),
                page,
                sort: sort.to_string(),
                ..CatalogFilter::default()
            })
            .await?
            .items)
    }

    pub async fn search_filtered(&self, filter: CatalogFilter) -> AppResult<CatalogListPage> {
        let mut filter = filter;
        let original_search = filter.search.clone();
        let original_creator = filter.creator.clone();
        let requested_page = filter.page.max(1);
        let requested_rows = filter.rows.clamp(30, 90);

        // Prefer apostrophe-stripped queries — SAM's index usually drops them.
        if !filter.search.trim().is_empty() {
            filter.search = text::prepare_sam_search_query(&filter.search);
        }
        if !filter.creator.trim().is_empty() {
            filter.creator = text::prepare_sam_search_query(&filter.creator);
        }

        let mut page = self.search_filtered_once(&filter).await?;
        tracing::info!(
            search = %filter.search,
            creator = %filter.creator,
            page = filter.page,
            sort = %filter.sort,
            date_days = filter.date_days,
            tags = filter.tags.len(),
            notags = filter.notags.len(),
            prefixes = filter.prefixes.len(),
            hits = page.items.len(),
            total_pages = page.total_pages,
            "SAM catalog list"
        );

        // Empty title search on page 1: retry query variants / alternate sorts.
        // Pagination must stay on a single query string so page 2+ stays consistent.
        if page.items.is_empty()
            && filter.page <= 1
            && !original_search.trim().is_empty()
            && filter.creator.trim().is_empty()
        {
            let variants = text::sam_search_variants(&original_search);
            let sorts: &[&str] = if filter.sort.eq_ignore_ascii_case("date") {
                &["likes", "name"]
            } else {
                &["date", "likes"]
            };

            'retry: for (i, variant) in variants.into_iter().enumerate().take(5) {
                if variant.eq_ignore_ascii_case(filter.search.trim()) {
                    continue;
                }
                let mut retry = filter.clone();
                retry.search = variant.clone();
                let hits = self.search_filtered_once(&retry).await?;
                if !hits.items.is_empty() {
                    tracing::info!(
                        original = %original_search,
                        variant = %variant,
                        sort = %retry.sort,
                        hits = hits.items.len(),
                        total_pages = hits.total_pages,
                        "SAM catalog list recovered via query variant"
                    );
                    page = hits;
                    break 'retry;
                }
                // Only fan out sort retries for the first couple of variants.
                if i >= 2 {
                    continue;
                }
                for sort in sorts {
                    let mut retry_sort = retry.clone();
                    retry_sort.sort = (*sort).to_string();
                    let hits = self.search_filtered_once(&retry_sort).await?;
                    if !hits.items.is_empty() {
                        tracing::info!(
                            original = %original_search,
                            variant = %variant,
                            sort = %sort,
                            hits = hits.items.len(),
                            total_pages = hits.total_pages,
                            "SAM catalog list recovered via variant + sort"
                        );
                        page = hits;
                        break 'retry;
                    }
                }
            }
        } else if page.items.is_empty()
            && filter.page <= 1
            && !original_creator.trim().is_empty()
        {
            let variants = text::sam_search_variants(&original_creator);
            for variant in variants.into_iter().take(4) {
                if variant.eq_ignore_ascii_case(filter.creator.trim()) {
                    continue;
                }
                let mut retry = filter.clone();
                retry.creator = variant.clone();
                let hits = self.search_filtered_once(&retry).await?;
                if !hits.items.is_empty() {
                    tracing::info!(
                        original = %original_creator,
                        variant = %variant,
                        hits = hits.items.len(),
                        total_pages = hits.total_pages,
                        "SAM catalog list recovered via creator variant"
                    );
                    page = hits;
                    break;
                }
            }
        }

        // Prefer the requested page/rows when SAM omits pagination metadata.
        if page.page == 0 {
            page.page = requested_page;
        }
        if page.rows == 0 {
            page.rows = requested_rows;
        }
        Ok(page)
    }

    async fn search_filtered_once(&self, filter: &CatalogFilter) -> AppResult<CatalogListPage> {
        let url = build_catalog_list_url(filter)?;
        tracing::debug!(%url, "SAM request");
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
        parse_list_page(&text, filter.page.max(1), filter.rows.clamp(30, 90))
    }

    /// Refresh tag id→name map from authenticated `cmd=options` (same source as the website UI).
    pub async fn fetch_tag_options(&self) -> AppResult<Option<std::collections::HashMap<i64, String>>> {
        let url = format!("{F95_LATEST_DATA_URL}?cmd=options");
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let text = response.text().await?;
        Ok(tags::parse_options_tags(&text))
    }

    pub async fn search_default(&self, query: &str, page: u32) -> AppResult<Vec<F95SearchResult>> {
        self.search(query, page, "date").await
    }

    pub async fn fetch_list_entry(&self, thread_id: i64) -> AppResult<Option<F95SearchResult>> {
        self.fetch_list_entry_with_hint(thread_id, "").await
    }

    /// Look up a thread in the SAM list API. Numeric id search often misses; title hint helps.
    pub async fn fetch_list_entry_with_hint(
        &self,
        thread_id: i64,
        title_hint: &str,
    ) -> AppResult<Option<F95SearchResult>> {
        // Prefer likes, then date — avoid hammering SAM with every sort (adds latency on live).
        for sort in ["likes", "date"] {
            let results = self.search(&thread_id.to_string(), 1, sort).await?;
            if let Some(hit) = results.into_iter().find(|r| r.thread_id == thread_id) {
                return Ok(Some(hit));
            }
        }

        let hint = text::clean_f95_title(title_hint);
        let hint = hint.trim();
        if hint.len() >= 3 {
            for query in text::sam_search_variants(hint) {
                for sort in ["likes", "date"] {
                    let results = self.search(&query, 1, sort).await?;
                    if let Some(hit) = results.into_iter().find(|r| r.thread_id == thread_id) {
                        tracing::debug!(thread_id, %query, %sort, "SAM list entry found via title hint");
                        return Ok(Some(hit));
                    }
                }
            }
        }

        Ok(None)
    }

    pub async fn fetch_thread_metadata(&self, thread_id: i64) -> AppResult<ThreadMetadata> {
        tracing::debug!(thread_id, "thread HTML fetch starting");
        let html = match tokio::time::timeout(
            std::time::Duration::from_secs(12),
            self.fetch_thread_html(thread_id),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                return Err(AppError::Other(format!(
                    "Timed out downloading F95 thread {thread_id}."
                )));
            }
        };
        let html_bytes = html.len();
        tracing::debug!(thread_id, html_bytes, "thread HTML downloaded; parsing off async runtime");

        // Parse can be CPU-heavy on large posts. Running it on the async worker prevents
        // tokio::time::timeout siblings from firing (executor starvation) — which matched
        // production hangs after "library add started" / "catalog preview started".
        let parse = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                parse_thread_html(thread_id, &html)
            }))
        })
        .await
        .map_err(|e| AppError::Other(format!("thread parse task failed: {e}")))?;

        match parse {
            Ok(result) => {
                tracing::debug!(thread_id, "thread HTML parse finished");
                result
            }
            Err(payload) => {
                let msg = panic_payload_message(&payload);
                tracing::error!(thread_id, panic = %msg, html_bytes, "parse_thread_html panicked");
                Err(AppError::Other(format!(
                    "Failed to parse F95 thread {thread_id}. Check F95 login and try again."
                )))
            }
        }
    }

    pub async fn fetch_thread_html(&self, thread_id: i64) -> AppResult<String> {
        let url = format!("{F95_BASE_URL}/threads/{thread_id}/");
        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(AppError::Other(format!(
                "failed to fetch thread {thread_id}: {}",
                response.status()
            )));
        }
        Ok(response.text().await?)
    }
}

/// Pull candidate game-download URLs from thread HTML and classify known hosts.
///
/// Only the OP is considered — reply posts are stripped first.
///
/// Walks the post in document order so nearby platform headings (`Windows`, `PC`)
/// and pack titles (`Episode 5`, `v0.4 Full`) attach to the following hoster links.
pub fn extract_download_links(html: &str) -> Vec<DownloadLink> {
    let html = isolate_op_and_header(html);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let lower = html.to_ascii_lowercase();
    let mut search_from = 0;
    let mut last_platform: Option<String> = None;
    let mut last_title: Option<String> = None;

    while let Some(rel) = find_next_download_event(&lower, &html, search_from) {
        match rel {
            DownloadWalkEvent::Platform { end, name } => {
                last_platform = Some(name);
                search_from = end;
            }
            DownloadWalkEvent::Section { end, title, platform } => {
                last_title = Some(title);
                // New pack heading resets platform unless the heading embeds one
                // ("v0.3 Legacy - Android"), so the prior section's OS does not leak.
                last_platform = platform;
                search_from = end;
            }
            DownloadWalkEvent::Href { start, end } => {
                search_from = end + 1;
                if end > html.len() || !html.is_char_boundary(start) || !html.is_char_boundary(end) {
                    continue;
                }
                let raw = html[start..end].trim();
                if raw.is_empty() || raw.starts_with('#') || raw.starts_with("javascript:") {
                    continue;
                }
                let url = if raw.starts_with("//") {
                    format!("https:{raw}")
                } else if raw.starts_with('/') {
                    format!("{F95_BASE_URL}{raw}")
                } else {
                    raw.to_string()
                };
                let host = classify_download_host(&url);
                if host == "skip" {
                    continue;
                }
                let key = url.to_ascii_lowercase();
                if !seen.insert(key) {
                    continue;
                }
                let platform = infer_platform_label(&url).or_else(|| last_platform.clone());
                out.push(DownloadLink {
                    url,
                    host,
                    label: platform,
                    title: last_title.clone(),
                });
                if out.len() >= 80 {
                    break;
                }
            }
        }
    }
    out
}

enum DownloadWalkEvent {
    Href { start: usize, end: usize },
    Platform { end: usize, name: String },
    Section {
        end: usize,
        title: String,
        platform: Option<String>,
    },
}

fn find_next_download_event(lower: &str, html: &str, from: usize) -> Option<DownloadWalkEvent> {
    let href = lower[from..].find("href=\"").map(|i| from + i);
    let plat = find_platform_marker(lower, from);
    let section = find_section_heading(lower, html, from);

    let mut best: Option<(usize, DownloadWalkEvent)> = None;
    let consider = |best: &mut Option<(usize, DownloadWalkEvent)>, at: usize, ev: DownloadWalkEvent| {
        let replace = match best {
            None => true,
            Some((bst, _)) => at < *bst,
        };
        if replace {
            *best = Some((at, ev));
        }
    };

    if let Some(h) = href {
        let start = h + 6;
        if let Some(rel_end) = lower[start..].find('"') {
            let end = start + rel_end;
            consider(&mut best, h, DownloadWalkEvent::Href { start, end });
        }
    }
    if let Some((start, end, name)) = plat {
        consider(
            &mut best,
            start,
            DownloadWalkEvent::Platform { end, name },
        );
    }
    if let Some((at, end, title, platform)) = section {
        consider(
            &mut best,
            at,
            DownloadWalkEvent::Section {
                end,
                title,
                platform,
            },
        );
    }
    best.map(|(_, ev)| ev)
}

fn find_platform_marker(lower: &str, from: usize) -> Option<(usize, usize, String)> {
    // Longer / compound markers first so "Windows/Linux" and "PC" are not reduced to Linux-only.
    const MARKERS: &[(&str, &str)] = &[
        ("windows/linux", "Windows/Linux"),
        ("linux/windows", "Windows/Linux"),
        ("win/linux", "Windows/Linux"),
        ("linux/win", "Windows/Linux"),
        ("windows / linux", "Windows/Linux"),
        ("linux / windows", "Windows/Linux"),
        ("windows & linux", "Windows/Linux"),
        ("linux & windows", "Windows/Linux"),
        ("windows and linux", "Windows/Linux"),
        ("linux and windows", "Windows/Linux"),
        ("pc / linux", "PC"),
        ("pc/linux", "PC"),
        ("linux / pc", "PC"),
        ("pc / windows", "PC"),
        ("windows / pc", "PC"),
        ("pc/mac", "PC/Mac"),
        ("pc / mac", "PC/Mac"),
        ("windows/mac", "Windows/Mac"),
        ("windows / mac", "Windows/Mac"),
        ("windows", "Windows"),
        ("linux", "Linux"),
        ("mac os", "Mac"),
        ("macos", "Mac"),
        ("osx", "Mac"),
        ("android", "Android"),
        ("pc", "PC"),
    ];
    let mut best: Option<(usize, usize, String)> = None; // (start, end, label)
    for (needle, label) in MARKERS {
        if let Some(i) = lower[from..].find(needle) {
            let at = from + i;
            let end = at + needle.len();
            // Require loose word boundaries for short tokens like "pc" / "osx".
            if *needle == "pc" || *needle == "osx" {
                let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
                let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
                if !before_ok || !after_ok {
                    continue;
                }
            }
            let better = match &best {
                None => true,
                Some((bst, bend, _)) => {
                    at < *bst || (at == *bst && end - at > *bend - *bst)
                }
            };
            if better {
                best = Some((at, end, (*label).to_string()));
            }
        }
    }
    best
}

/// Next bold/strong heading that looks like a download-pack title
/// (Episode 5, v0.4 Full, Update only) rather than a platform or overview label.
fn find_section_heading(
    lower: &str,
    html: &str,
    from: usize,
) -> Option<(usize, usize, String, Option<String>)> {
    let mut search = from;
    while search < lower.len() {
        let (tag_at, open_len, close) = if let Some(i) = lower[search..].find("<b>") {
            (search + i, 3usize, "</b>")
        } else if let Some(i) = lower[search..].find("<strong>") {
            (search + i, 8usize, "</strong>")
        } else if let Some(i) = lower[search..].find("<b ") {
            let at = search + i;
            let Some(gt) = lower[at..].find('>') else {
                search = at + 2;
                continue;
            };
            (at, gt + 1, "</b>")
        } else {
            return None;
        };

        let content_start = tag_at + open_len;
        let Some(rel_close) = lower[content_start..].find(close) else {
            search = content_start;
            continue;
        };
        let content_end = content_start + rel_close;
        let end = content_end + close.len();
        search = end;

        if content_end > html.len()
            || !html.is_char_boundary(content_start)
            || !html.is_char_boundary(content_end)
        {
            continue;
        }
        let raw = html[content_start..content_end].trim();
        let cleaned = strip_simple_html(raw);
        let cleaned = text::decode_html_entities(&cleaned);
        let cleaned = collapse_heading_ws(&cleaned);
        if cleaned.is_empty() {
            continue;
        }

        if heading_is_platform_only(&cleaned) {
            continue;
        }
        if heading_is_ignored(&cleaned) {
            continue;
        }

        let (title, platform) = split_heading_title_and_platform(&cleaned);
        if title.chars().count() < 2 || title.chars().count() > 80 {
            continue;
        }
        return Some((tag_at, end, title, platform));
    }
    None
}

fn strip_simple_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn collapse_heading_ws(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn heading_is_platform_only(text: &str) -> bool {
    let t = text.trim().trim_end_matches(':').trim();
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        "windows"
        | "linux"
        | "mac"
        | "macos"
        | "mac os"
        | "osx"
        | "android"
        | "pc"
        | "windows/linux"
        | "linux/windows"
        | "windows / linux"
        | "linux / windows"
        | "windows & linux"
        | "linux & windows"
        | "windows and linux"
        | "linux and windows"
        | "pc/mac"
        | "pc / mac"
        | "windows/mac"
        | "windows / mac"
        | "win"
        | "pc / linux"
        | "pc/linux" => true,
        _ => {
            // "Windows / Linux builds" still platform-ish if no other title words remain.
            infer_platform_from_heading_text(t).is_some() && !heading_has_extra_title_words(t)
        }
    }
}

fn heading_has_extra_title_words(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let stripped = lower
        .replace("windows", " ")
        .replace("linux", " ")
        .replace("macos", " ")
        .replace("mac os", " ")
        .replace("android", " ")
        .replace("osx", " ")
        .replace("mac", " ")
        .replace("win", " ")
        .replace("pc", " ")
        .replace('/', " ")
        .replace('&', " ")
        .replace('+', " ")
        .replace('|', " ")
        .replace('-', " ")
        .replace(':', " ")
        .replace('(', " ")
        .replace(')', " ");
    stripped.split_whitespace().any(|w| {
        !w.is_empty()
            && w != "and"
            && w != "or"
            && w != "build"
            && w != "builds"
            && w != "only"
    })
}

fn heading_is_ignored(text: &str) -> bool {
    let t = text.trim().trim_end_matches(':').trim().to_ascii_lowercase();
    matches!(
        t.as_str(),
        "download"
            | "downloads"
            | "download links"
            | "download link"
            | "links"
            | "mirrors"
            | "mirror"
            | "host"
            | "hosts"
            | "hosters"
            | "mega"
            | "gofile"
            | "pixeldrain"
            | "datanodes"
            | "mediafire"
            | "dropbox"
            | "developer"
            | "artist"
            | "publisher"
            | "studio"
            | "creator"
            | "author"
            | "version"
            | "engine"
            | "os"
            | "platform"
            | "platforms"
            | "censorship"
            | "language"
            | "languages"
            | "genre"
            | "tags"
            | "overview"
            | "thread updated"
            | "release date"
            | "last updated"
            | "spoiler"
            | "show"
            | "hide"
            | "quote"
            | "code"
    ) || t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("www.")
}

fn infer_platform_from_heading_text(text: &str) -> Option<String> {
    let lower = text.trim().trim_end_matches(':').trim().to_ascii_lowercase();
    if lower.contains("windows") && lower.contains("linux") {
        return Some("Windows/Linux".into());
    }
    if lower.contains("windows") && lower.contains("mac") {
        return Some("Windows/Mac".into());
    }
    if lower.contains("windows") {
        return Some("Windows".into());
    }
    if lower.contains("linux") {
        return Some("Linux".into());
    }
    if lower.contains("android") {
        return Some("Android".into());
    }
    if lower.contains("macos") || lower.contains("osx") || lower.contains("mac os") || lower == "mac"
    {
        return Some("Mac".into());
    }
    if find_platform_marker(&lower, 0).is_some_and(|(_, _, name)| name == "PC")
        && !heading_has_extra_title_words(text)
    {
        return Some("PC".into());
    }
    None
}

fn split_heading_title_and_platform(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim().trim_end_matches(':').trim();
    for sep in [" - ", " – ", " — ", " | ", " / ", " · "] {
        if let Some((left, right)) = trimmed.split_once(sep) {
            let left = left.trim();
            let right = right.trim();
            if heading_is_platform_only(right) && !heading_is_platform_only(left) {
                return (
                    left.to_string(),
                    infer_platform_from_heading_text(right).or_else(|| Some(right.to_string())),
                );
            }
            if heading_is_platform_only(left) && !heading_is_platform_only(right) {
                return (
                    right.to_string(),
                    infer_platform_from_heading_text(left).or_else(|| Some(left.to_string())),
                );
            }
        }
    }
    for (open, close) in [('(', ')'), ('[', ']')] {
        if let Some(start) = trimmed.rfind(open) {
            if let Some(end) = trimmed[start..].find(close) {
                let inner = trimmed[start + 1..start + end].trim();
                let outer = trimmed[..start].trim();
                if !outer.is_empty() && heading_is_platform_only(inner) {
                    return (
                        outer.to_string(),
                        infer_platform_from_heading_text(inner).or_else(|| Some(inner.to_string())),
                    );
                }
            }
        }
    }
    (trimmed.to_string(), None)
}

fn infer_platform_label(url: &str) -> Option<String> {
    let u = url.to_ascii_lowercase();
    let has_win = u.contains("-win")
        || u.contains("_win")
        || u.contains("/win/")
        || u.contains("windows")
        || u.contains("win32")
        || u.contains("win64");
    let has_linux = u.contains("-linux") || u.contains("_linux") || u.contains("/linux/") || u.contains("linux");
    let has_mac = u.contains("-mac")
        || u.contains("_mac")
        || u.contains("/mac/")
        || u.contains("macos")
        || u.contains("osx");
    let has_android = u.contains("android") || u.ends_with(".apk") || u.contains(".apk?");
    let has_pc = u.contains("/pc/") || u.contains("-pc") || u.contains("_pc") || u.contains(" pc.");

    if (has_win && has_linux) || has_pc {
        return Some(if has_pc && !has_win && !has_linux {
            "PC".into()
        } else {
            "Windows/Linux".into()
        });
    }
    if has_win {
        Some("Windows".into())
    } else if has_linux {
        Some("Linux".into())
    } else if has_mac {
        Some("Mac".into())
    } else if has_android {
        Some("Android".into())
    } else {
        None
    }
}

fn classify_download_host(url: &str) -> String {
    let u = url.to_ascii_lowercase();
    // F95 masks external hosters: /masked/pixeldrain.com/... or masked-navigation?t=
    if let Some(target) = extract_masked_target_host(&u) {
        return classify_download_host(&format!("https://{target}/"));
    }
    // Skip page chrome / site assets (whole-page href scrape otherwise returns CSS/JS/fonts).
    if is_non_download_asset(&u)
        || u.contains("f95zone.to/threads/")
        || u.contains("f95zone.to/members/")
        || u.contains("f95zone.to/login")
        || u.contains("f95zone.to/styles/")
        || u.contains("f95zone.to/data/")
        || u.contains("attachments.f95zone.to")
        || u.contains("imgur.com")
        || u.contains("discord.com")
        || u.contains("discordapp.com")
        || u.contains("patreon.com")
        || u.contains("subscribestar")
    {
        return "skip".into();
    }
    if u.contains("gofile.io") {
        return "gofile".into();
    }
    if u.contains("mega.nz") || u.contains("mega.co.nz") {
        return "mega".into();
    }
    if u.contains("pixeldrain.com") {
        return "pixeldrain".into();
    }
    if u.contains("datanodes.to") {
        return "datanodes".into();
    }
    if u.contains("buzzheavier.com")
        || u.contains("mixdrop.")
        || u.contains("uploadhaven.com")
        || u.contains("mediafire.com")
        || u.contains("workupload.com")
        || u.contains("drive.google.com")
        || u.contains("dropbox.com")
        || u.contains("catbox.moe")
        || u.contains("bunkr.")
        || u.contains("anonfiles")
    {
        return "http".into();
    }
    if (u.starts_with("http://") || u.starts_with("https://")) && looks_like_archive_download(&u) {
        // Direct archive / installer links — useful for Afterglow's HTTP adapter.
        return "http".into();
    }
    "skip".into()
}

fn extract_masked_target_host(u: &str) -> Option<String> {
    const MARKER: &str = "f95zone.to/masked/";
    if let Some(idx) = u.find(MARKER) {
        let rest = &u[idx + MARKER.len()..];
        let host = rest.split('/').next().unwrap_or("").trim();
        if !host.is_empty() && host.contains('.') {
            return Some(host.to_string());
        }
    }
    None
}

fn is_non_download_asset(u: &str) -> bool {
    const EXTS: &[&str] = &[
        ".css", ".js", ".mjs", ".map", ".woff", ".woff2", ".ttf", ".otf", ".eot", ".svg", ".ico",
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".mp4", ".webm", ".json",
    ];
    if EXTS.iter().any(|ext| {
        u.ends_with(ext)
            || u.contains(&format!("{ext}?"))
            || u.contains(&format!("{ext}#"))
    }) {
        return true;
    }
    u.contains("/css/")
        || u.contains("/js/")
        || u.contains("/fonts/")
        || u.contains("fontawesome")
        || u.contains("cdnjs.")
        || u.contains("googleapis.com")
        || u.contains("gstatic.com")
}

fn looks_like_archive_download(u: &str) -> bool {
    const EXTS: &[&str] = &[
        ".zip", ".rar", ".7z", ".exe", ".apk", ".tar", ".gz", ".bz2", ".xz", ".iso",
    ];
    EXTS.iter()
        .any(|ext| u.ends_with(ext) || u.contains(&format!("{ext}?")) || u.contains(&format!("{ext}&")))
}

fn parse_list_response(text: &str) -> AppResult<Vec<F95SearchResult>> {
    Ok(parse_list_page(text, 1, 90)?.items)
}

fn parse_list_page(text: &str, fallback_page: u32, fallback_rows: u32) -> AppResult<CatalogListPage> {
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

    let msg = body.msg.unwrap_or(F95ListMessage {
        data: Vec::new(),
        pagination: None,
    });
    let items: Vec<F95SearchResult> = msg.data.into_iter().map(item_to_result).collect();
    let (page, total_pages) = match msg.pagination {
        Some(p) => (
            if p.page > 0 { p.page } else { fallback_page },
            p.total,
        ),
        None => {
            // Heuristic when SAM omits pagination: full page ⇒ assume at least one more.
            let total = if (items.len() as u32) >= fallback_rows {
                fallback_page.saturating_add(1)
            } else {
                fallback_page.max(1)
            };
            (fallback_page.max(1), total)
        }
    };

    Ok(CatalogListPage {
        items,
        page,
        total_pages,
        rows: fallback_rows,
    })
}

fn normalize_sort(sort: &str) -> &'static str {
    match sort.trim().to_lowercase().as_str() {
        "likes" | "like" => "likes",
        "views" | "view" => "views",
        "name" | "title" | "az" | "a-z" => "name",
        // F95 "weighted rating"
        "rating" | "rate" | "weighted" | "weighted_rating" => "rating",
        _ => "date",
    }
}

/// Build the F95 SAM `latest_data.php` list URL.
///
/// Tag **names** are resolved to numeric IDs (SAM ignores names). Already-numeric
/// IDs (e.g. from `AppState::catalog_search`) pass through unchanged so sort/date
/// keep working with the hub's fuller tag map.
pub fn build_catalog_list_url(filter: &CatalogFilter) -> AppResult<String> {
    let sort = normalize_sort(&filter.sort);
    let page = filter.page.max(1);
    let mut url = format!(
        "{F95_LATEST_DATA_URL}?cmd=list&cat=games&sort={sort}&page={page}&rows={}",
        filter.rows.max(1).min(90)
    );

    let search = text::prepare_sam_search_query(&filter.search);
    if !search.is_empty() {
        url.push_str(&format!("&search={}", urlencoding::encode(&search)));
    }
    let creator = text::prepare_sam_search_query(&filter.creator);
    if !creator.is_empty() {
        url.push_str(&format!("&creator={}", urlencoding::encode(&creator)));
    }
    if filter.date_days > 0 {
        url.push_str(&format!("&date={}", filter.date_days));
    }
    if filter.tag_mode.eq_ignore_ascii_case("or") {
        url.push_str("&tagtype=or");
    }

    let catalog = TagCatalog::seed();
    let tag_ids = resolve_sam_tag_tokens(&catalog, &filter.tags, "tag")?;
    let notag_ids = resolve_sam_tag_tokens(&catalog, &filter.notags, "exclude tag")?;

    for tag in &tag_ids {
        // Percent-encode brackets so URL parsers cannot strip PHP array params.
        url.push_str(&format!("&tags%5B%5D={}", tag));
    }
    for tag in &notag_ids {
        url.push_str(&format!("&notags%5B%5D={}", tag));
    }
    for prefix in &filter.prefixes {
        if !prefix.is_empty() {
            url.push_str(&format!("&prefixes%5B%5D={}", urlencoding::encode(prefix)));
        }
    }
    Ok(url)
}

fn resolve_sam_tag_tokens(
    catalog: &TagCatalog,
    tokens: &[String],
    kind: &str,
) -> AppResult<Vec<String>> {
    if text::looks_like_tag_ids(tokens) || tokens.is_empty() {
        return Ok(tokens
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect());
    }
    match catalog.resolve_query_list(tokens) {
        Ok(ids) => Ok(ids),
        Err(unknown) => Err(AppError::BadRequest(format!(
            "Unknown F95 {kind}(s): {}. Use names from the Browse tag list (e.g. female protagonist).",
            unknown.join(", ")
        ))),
    }
}

/// One page of SAM catalog results, including F95 pagination totals when present.
#[derive(Debug, Clone)]
pub struct CatalogListPage {
    pub items: Vec<F95SearchResult>,
    pub page: u32,
    pub total_pages: u32,
    pub rows: u32,
}

impl CatalogListPage {
    pub fn has_more(&self) -> bool {
        if self.total_pages > 0 {
            self.page < self.total_pages
        } else {
            (self.items.len() as u32) >= self.rows.max(1)
        }
    }
}

/// Query options mirroring F95Zone SAM `latest_data.php` list filters.
#[derive(Debug, Clone)]
pub struct CatalogFilter {
    pub search: String,
    pub creator: String,
    pub page: u32,
    pub rows: u32,
    pub sort: String,
    /// Updated within N days (0 = any time). Maps to F95 `date` param.
    pub date_days: u32,
    pub tag_mode: String,
    pub tags: Vec<String>,
    pub notags: Vec<String>,
    pub prefixes: Vec<String>,
}

impl Default for CatalogFilter {
    fn default() -> Self {
        Self {
            search: String::new(),
            creator: String::new(),
            page: 1,
            // SAM ignores rows < 30 (still returns 30); 90 is the practical page size.
            rows: 90,
            sort: "date".into(),
            date_days: 0,
            tag_mode: "and".into(),
            tags: Vec::new(),
            notags: Vec::new(),
            prefixes: Vec::new(),
        }
    }
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

    let prefixes = {
        let from_title = text::extract_title_prefixes(&item.title);
        if !from_title.is_empty() {
            from_title
        } else {
            item.prefixes
                .unwrap_or_default()
                .into_iter()
                .filter(|p| !p.chars().all(|c| c.is_ascii_digit()))
                .collect()
        }
    };

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
        // Keep raw SAM tag IDs here; AppState::catalog_search maps them with the full tag catalog.
        tags: item.tags.unwrap_or_default(),
        prefixes,
        platforms: Vec::new(),
        rating: item.rating.unwrap_or(0.0),
        likes: item.likes,
        views: item.views,
        url: format!("{F95_BASE_URL}/threads/{}/", item.thread_id),
        date: item.date.unwrap_or_default(),
    }
}

fn parse_thread_html(thread_id: i64, html: &str) -> AppResult<ThreadMetadata> {
    // Only the thread header + OP matter. Reply pages are huge and irrelevant.
    let html = isolate_op_and_header(html);

    let raw_title = extract_thread_title(&html)
        .or_else(|| extract_meta_content(&html, "og:title"))
        .or_else(|| extract_tag_text(&html, "h1"))
        .unwrap_or_else(|| format!("Thread {thread_id}"));

    let title = text::clean_f95_title(&raw_title);

    let og_cover = extract_meta_content(&html, "og:image").unwrap_or_default();
    let description = extract_first_post_description(&html).or_else(|| {
        extract_meta_content(&html, "og:description")
            .map(|d| text::decode_html_entities(&d))
            .filter(|d| !d.ends_with("..."))
    });
    let post_images = extract_first_post_images(&html);
    let (cover, screenshots) = text::split_cover_and_screenshots(&post_images);
    let cover = if cover.is_empty() {
        text::pick_best_cover(&og_cover, &post_images)
    } else {
        cover
    };
    let rating = extract_rating(&html);

    Ok(ThreadMetadata {
        result: F95SearchResult {
            thread_id,
            title,
            creator: extract_creator(&html),
            version: extract_version(&html),
            cover: cover.clone(),
            screenshots: screenshots.clone(),
            tags: extract_tags(&html),
            prefixes: text::extract_title_prefixes(&raw_title),
            platforms: extract_platforms(&html),
            rating,
            likes: None,
            views: None,
            url: format!("{F95_BASE_URL}/threads/{thread_id}/"),
            date: String::new(),
        },
        screenshots,
        all_images: post_images,
        description,
    })
}

/// Keep page chrome (title / tags / rating) + the OP article; drop every reply.
///
/// F95 thread pages can be multi‑MB once replies are included. Game metadata and
/// download links live exclusively in the starter post (and header above it).
fn isolate_op_and_header(html: &str) -> String {
    let markers = ["message-threadStarterPost", "threadStarterPost"];
    for marker in markers {
        if let Some(idx) = html.find(marker) {
            let article_start = html[..idx].rfind("<article").unwrap_or(idx);
            let after = &html[article_start..];
            if let Some(rel_end) = after.find("</article>") {
                let end = article_start + rel_end + "</article>".len();
                // From document start through OP close — includes title/tags above posts.
                return html[..end].to_string();
            }
        }
    }

    // Fallback when XenForo markers are missing (login wall / layout change).
    if let Some(idx) = html.find("class=\"message-body") {
        let end = (idx + 300_000).min(html.len());
        let end = floor_char_boundary(html, end);
        return html[..end].to_string();
    }

    let end = floor_char_boundary(html, html.len().min(250_000));
    html[..end].to_string()
}

fn extract_thread_title(html: &str) -> Option<String> {
    for marker in ["p-title-value", "thread-title"] {
        if let Some(idx) = html.find(marker) {
            let slice = safe_slice(html, idx, idx + 500);
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

/// Pull F95 star rating from thread HTML (schema.org / XenForo ld+json).
fn extract_rating(html: &str) -> f64 {
    if let Some(v) = extract_json_number_field(html, "ratingValue") {
        if (0.0..=5.0).contains(&v) && v > 0.0 {
            return v;
        }
    }
    // Fallback: br-rating / ratingValue attributes sometimes present as data attrs
    for attr in ["data-rating", "data-xf-init=\"rating\""] {
        let _ = attr;
    }
    if let Some(idx) = html.find("br-rating") {
        let slice = safe_slice(html, idx, idx + 400);
        if let Some(v) = extract_json_number_field(slice, "rating") {
            if (0.0..=5.0).contains(&v) && v > 0.0 {
                return v;
            }
        }
        // title="4.50 / 5" style
        if let Some(t) = slice.find("title=\"") {
            let rest = &slice[t + 7..];
            if let Some(end) = rest.find('"') {
                let title = &rest[..end];
                if let Some(num) = title.split('/').next() {
                    if let Ok(v) = num.trim().parse::<f64>() {
                        if (0.0..=5.0).contains(&v) && v > 0.0 {
                            return v;
                        }
                    }
                }
            }
        }
    }
    0.0
}

fn extract_json_number_field(html: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\"");
    let mut search = html;
    while let Some(idx) = search.find(&needle) {
        let mut after = search[idx + needle.len()..].trim_start();
        after = after.strip_prefix(':')?.trim_start();
        let num = if let Some(rest) = after.strip_prefix('"') {
            let end = rest.find('"')?;
            rest[..end].trim()
        } else {
            let end = after
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
                .unwrap_or(after.len());
            after[..end].trim()
        };
        if let Ok(v) = num.parse::<f64>() {
            return Some(v);
        }
        search = &search[idx + needle.len()..];
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
        text = safe_slice(&text, 0, end).trim().to_string();
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

    // ASCII lowercase keeps byte indexes aligned with `text` (Unicode to_lowercase does not —
    // e.g. ß→ss expands and `text[start..]` panics → Cloudflare 502 with no JSON body).
    let lower = text.to_ascii_lowercase();
    let start = lower
        .find("overview:")
        .or_else(|| lower.find("**overview:**"))?;
    let start = floor_char_boundary(&text, start);
    let from_overview = safe_slice(&text, start, text.len());
    let end = find_thread_updated_marker(from_overview).unwrap_or(from_overview.len());
    let end = floor_char_boundary(from_overview, end);
    let mut slice = safe_slice(from_overview, 0, end).trim().to_string();

    let slice_lower = slice.to_ascii_lowercase();
    if slice_lower.starts_with("overview:") {
        slice = safe_slice(&slice, "overview:".len(), slice.len())
            .trim()
            .to_string();
    } else if slice_lower.starts_with("**overview:**") {
        slice = safe_slice(&slice, "**overview:**".len(), slice.len())
            .trim()
            .to_string();
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
    let lower = text.to_ascii_lowercase();
    for marker in ["thread updated:", "thread update:"] {
        if let Some(idx) = lower.find(marker) {
            return Some(floor_char_boundary(text, idx));
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
            .or_else(|| extract_attr(fragment, "src"))
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

/// Soft ceiling so a malformed scrape cannot explode DB/media rows. Real F95
/// galleries are typically well under this (25–40); raise if needed.
pub const MAX_THREAD_IMAGES: usize = 80;

fn dedupe_upgraded_images(images: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for url in images {
        let key = url.to_lowercase();
        if seen.insert(key) {
            out.push(url);
        }
    }
    out.truncate(MAX_THREAD_IMAGES);
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

    // Fallback: first message-body block (large posts can exceed 80KB of HTML).
    let idx = html.find("class=\"message-body")?;
    let end = floor_char_boundary(html, (idx + 400_000).min(html.len()));
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
    // Prefer overview-style fields in the thread body. Avoid false positives from
    // XenForo "Game Developer" user banners.
    for label in ["Developer", "Artist", "Publisher", "Studio", "Creator", "Author"] {
        if let Some(name) = extract_labeled_field(html, label) {
            return name;
        }
    }

    // First post author attribute is a reasonable fallback for solo-dev threads.
    if let Some(author) = extract_first_post_author(html) {
        return author;
    }

    "Unknown".into()
}

fn extract_platforms(html: &str) -> Vec<String> {
    for label in [
        "Operating System",
        "Supported OS",
        "Platform",
        "Platforms",
        "Systems",
        "OS",
    ] {
        if let Some(raw) = extract_labeled_field_raw(html, label) {
            let platforms = text::parse_platforms(&raw);
            if !platforms.is_empty() {
                return platforms;
            }
        }
    }
    Vec::new()
}

/// Like `extract_labeled_field`, but returns the raw value text (for multi-value fields like OS).
fn extract_labeled_field_raw(html: &str, label: &str) -> Option<String> {
    // Must use ASCII lowercase so byte indexes still line up with `html`.
    // Unicode `to_lowercase()` expands chars (e.g. ß→ss) and makes `html[idx..]` panic,
    // which surfaces as an immediate Cloudflare 502 with no JSON body.
    let lower = html.to_ascii_lowercase();
    let label_l = label.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(rel) = lower[search_from..].find(&label_l) {
        let idx = search_from + rel;
        search_from = idx + label_l.len();

        let ctx = safe_slice(&lower, idx.saturating_sub(40), idx + label_l.len() + 40);
        if ctx.contains("userbanner")
            || ctx.contains("jobtitle")
            || ctx.contains("message-user")
            || ctx.contains("creator of")
        {
            continue;
        }

        // Avoid matching labels inside longer words (e.g. "OS" in "most", "Platform" in "Platformer").
        let before = lower[..idx].chars().rev().next().unwrap_or(' ');
        let after = lower[idx + label_l.len()..].chars().next().unwrap_or(' ');
        if before.is_ascii_alphanumeric() || after.is_ascii_alphanumeric() {
            continue;
        }

        let snippet = safe_slice(html, idx, idx + 320);

        if let Some(colon) = snippet.find(':') {
            let after = snippet[colon + 1..].trim_start();
            if let Some(value) = extract_raw_field_value(after) {
                return Some(value);
            }
        }

        if let Some(close_b) = snippet.find("</b>") {
            let after = snippet[close_b + 4..].trim_start();
            let after = after.strip_prefix(':').unwrap_or(after).trim_start();
            if let Some(value) = extract_raw_field_value(after) {
                return Some(value);
            }
        }
    }
    None
}

fn extract_raw_field_value(after: &str) -> Option<String> {
    let after = after.trim_start();
    if after.is_empty() {
        return None;
    }

    // Take until a line break / next overview field.
    let cut = after
        .find("<br")
        .or_else(|| after.find("</p>"))
        .or_else(|| after.find("</li>"))
        .or_else(|| after.find('\n'))
        .unwrap_or_else(|| floor_char_boundary(after, after.len().min(200)));
    let chunk = safe_slice(after, 0, cut);

    // Strip simple HTML tags while keeping link text.
    let mut text = String::new();
    let mut in_tag = false;
    for c in chunk.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text::decode_html_entities(text.trim());
    let text = text
        .trim_matches(|c: char| c == '-' || c == '–' || c == '—' || c == ':' || c == ',')
        .trim()
        .to_string();
    if text.is_empty() || text.len() > 160 {
        None
    } else {
        Some(text)
    }
}

fn extract_labeled_field(html: &str, label: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let label_l = label.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(rel) = lower[search_from..].find(&label_l) {
        let idx = search_from + rel;
        search_from = idx + label_l.len();

        // Skip user-banner / role badge hits like "Game Developer".
        let ctx = safe_slice(&lower, idx.saturating_sub(40), idx + label_l.len() + 40);
        if ctx.contains("userbanner")
            || ctx.contains("jobtitle")
            || ctx.contains("message-user")
            || ctx.contains("creator of")
        {
            continue;
        }

        let snippet = safe_slice(html, idx, idx + 280);

        // Developer:</b> <a>Name</a>  or  Developer: Name - Patreon
        if let Some(colon) = snippet.find(':') {
            let after = snippet[colon + 1..].trim_start();
            if let Some(name) = extract_name_after_label_value(after) {
                return Some(name);
            }
        }

        // <b>Developer</b> Name / <b>Developer</b><a>...
        if let Some(close_b) = snippet.find("</b>") {
            let after = snippet[close_b + 4..].trim_start();
            // optional colon already handled above; strip leftover colon
            let after = after.strip_prefix(':').unwrap_or(after).trim_start();
            if let Some(name) = extract_name_after_label_value(after) {
                return Some(name);
            }
        }
    }
    None
}

fn extract_name_after_label_value(after: &str) -> Option<String> {
    let after = after.trim_start();
    if after.is_empty() {
        return None;
    }

    // Prefer linked name when present.
    if let Some(rest) = after.strip_prefix("<a ") {
        if let Some(gt) = rest.find('>') {
            let inner = &rest[gt + 1..];
            if let Some(end) = inner.find("</a>") {
                let name = clean_creator_name(&inner[..end]);
                if is_plausible_creator(&name) {
                    return Some(name);
                }
            }
        }
    }

    // Plain text until break / link list separator.
    let raw = after
        .split(['<', '\n'])
        .next()
        .unwrap_or(after)
        .trim();
    let name = clean_creator_name(raw);
    if is_plausible_creator(&name) {
        Some(name)
    } else {
        None
    }
}

fn clean_creator_name(raw: &str) -> String {
    let decoded = text::decode_html_entities(raw);
    // "Mr_Fable - Steam - Patreon - SubscribeStar" → "Mr_Fable"
    let mut cut = decoded
        .split(" - ")
        .next()
        .unwrap_or(&decoded)
        .split('|')
        .next()
        .unwrap_or(&decoded)
        .trim()
        .trim_end_matches([':', '-', '–', '—'])
        .trim()
        .to_string();

    // "Paper Tiger Discord" / "Name Patreon" without separators
    for platform in [
        " Discord",
        " Patreon",
        " Steam",
        " SubscribeStar",
        " Subscribestar",
        " Itch.io",
        " Itch",
        " Fanbox",
        " Ci-en",
        " Twitter",
    ] {
        if let Some(idx) = cut.find(platform) {
            cut = cut[..idx].trim().to_string();
            break;
        }
    }
    cut
}

fn is_plausible_creator(name: &str) -> bool {
    let t = name.trim();
    if t.len() < 2 || t.len() > 80 {
        return false;
    }
    if t.eq_ignore_ascii_case("unknown") || t.eq_ignore_ascii_case("n/a") {
        return false;
    }
    // Reject leftover markup crumbs / pure punctuation.
    if !t.chars().any(|c| c.is_alphanumeric()) {
        return false;
    }
    true
}

fn extract_first_post_author(html: &str) -> Option<String> {
    // data-author="Paper Tiger 83" on the first message article
    let marker = "data-author=\"";
    let idx = html.find(marker)?;
    let rest = &html[idx + marker.len()..];
    let end = rest.find('"')?;
    let name = clean_creator_name(&rest[..end]);
    if is_plausible_creator(&name) {
        Some(name)
    } else {
        None
    }
}

/// Treat placeholder creator strings as missing.
pub fn normalize_creator(value: &str) -> Option<String> {
    let name = clean_creator_name(value);
    if is_plausible_creator(&name) {
        Some(name)
    } else {
        None
    }
}

fn extract_version(html: &str) -> String {
    for marker in ["Version", "version"] {
        if let Some(idx) = html.find(marker) {
            let snippet = safe_slice(html, idx, idx + 100);
            for part in snippet.split(|c: char| c == '<' || c == '>') {
                let trimmed = part.trim();
                if trimmed.chars().any(|c| c.is_ascii_digit()) && trimmed.len() < 30 {
                    let cleaned = trimmed
                        .trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_')
                        .to_string();
                    if !cleaned.is_empty() {
                        return cleaned;
                    }
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

/// Fast path for add/refresh: download cover only (no screenshot gallery).
pub async fn cache_thread_cover(
    db: &Database,
    client: &F95Client,
    game_id: i64,
    thread_id: i64,
    cover_url: &str,
    screenshots: &[String],
) -> AppResult<Option<String>> {
    match tokio::time::timeout(
        Duration::from_secs(4),
        cache_thread_cover_inner(db, client, game_id, thread_id, cover_url, screenshots),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(thread_id, "cover download timed out");
            Ok(None)
        }
    }
}

async fn cache_thread_cover_inner(
    db: &Database,
    client: &F95Client,
    game_id: i64,
    thread_id: i64,
    cover_url: &str,
    screenshots: &[String],
) -> AppResult<Option<String>> {
    let media_dir = db.media_dir().join(format!("{thread_id}"));
    std::fs::create_dir_all(&media_dir)?;

    let upgraded_screenshots: Vec<String> = screenshots
        .iter()
        .filter_map(|s| text::download_media_url(s).or_else(|| text::sam_list_media_url(s)))
        .collect();
    let cover_candidate = text::download_media_url(cover_url)
        .or_else(|| text::sam_list_media_url(cover_url))
        .unwrap_or_default();
    let effective_cover = text::pick_best_cover(&cover_candidate, &upgraded_screenshots);
    if effective_cover.is_empty() {
        return Ok(None);
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(7);
    let Some((path, resolved)) =
        download_image(client, &effective_cover, &media_dir, "cover", deadline).await
    else {
        return Ok(None);
    };

    // Replace prior cover row if present; leave existing screenshots alone.
    let _ = db.clear_game_cover_media(game_id);
    db.insert_media(game_id, &resolved, &path, "cover")?;
    Ok(Some(path))
}

/// Download screenshot gallery into hub media (does not touch the cover).
/// Intended for background use after add/refresh so the API stays fast.
///
/// Persists the **full** screenshot URL list (stubs for anything still pending)
/// so clients never lose images after a partial cache run.
pub async fn cache_thread_screenshots(
    db: &Database,
    client: &F95Client,
    game_id: i64,
    thread_id: i64,
    cover_url: &str,
    screenshots: &[String],
) -> AppResult<usize> {
    const MEDIA_BUDGET: Duration = Duration::from_secs(180);
    const DOWNLOAD_CONCURRENCY: usize = 4;

    let deadline = tokio::time::Instant::now() + MEDIA_BUDGET;
    let media_dir = db.media_dir().join(format!("{thread_id}"));
    std::fs::create_dir_all(&media_dir)?;

    let upgraded_screenshots: Vec<String> = screenshots
        .iter()
        .filter_map(|s| text::download_media_url(s).or_else(|| text::sam_list_media_url(s)))
        .collect();
    let cover_candidate = text::download_media_url(cover_url)
        .or_else(|| text::sam_list_media_url(cover_url))
        .unwrap_or_default();
    let effective_cover = text::pick_best_cover(&cover_candidate, &upgraded_screenshots);
    let cover_key = if effective_cover.is_empty() {
        String::new()
    } else {
        text::upgrade_image_url(&effective_cover)
    };

    // Ordered unique gallery URLs (cover excluded — it lives in the cover row).
    let mut seen = std::collections::HashSet::new();
    let gallery: Vec<String> = upgraded_screenshots
        .into_iter()
        .filter(|u| !u.is_empty() && !text::is_branding_image(u))
        .map(|u| text::upgrade_image_url(&u))
        .filter(|u| {
            if u.is_empty() {
                return false;
            }
            if !cover_key.is_empty() && u == &cover_key {
                return false;
            }
            seen.insert(u.to_lowercase())
        })
        .take(MAX_THREAD_IMAGES)
        .collect();

    if gallery.is_empty() {
        tracing::warn!(game_id, thread_id, "screenshot cache: empty gallery URL list");
        return Ok(0);
    }

    // Reuse already-cached files for the same source URL when present.
    let existing_by_url: std::collections::HashMap<String, String> = db
        .list_game_media(game_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.media_type == "screenshot")
        .filter_map(|m| {
            let path = m.local_path.filter(|p| !p.trim().is_empty())?;
            if !std::path::Path::new(&path).is_file() {
                return None;
            }
            Some((text::upgrade_image_url(&m.source_url).to_lowercase(), path))
        })
        .collect();

    let mut paths: Vec<Option<String>> = vec![None; gallery.len()];
    for (i, url) in gallery.iter().enumerate() {
        if let Some(path) = existing_by_url.get(&url.to_lowercase()) {
            paths[i] = Some(path.clone());
        }
    }

    let need_download: Vec<(usize, String)> = gallery
        .iter()
        .enumerate()
        .filter(|(i, _)| paths[*i].is_none())
        .map(|(i, u)| (i, u.clone()))
        .collect();

    // Parallel download with a small worker pool (still respects overall budget).
    let mut next = 0usize;
    let mut in_flight = FuturesUnordered::new();
    let spawn_one = |idx: usize, url: String| {
        let client = client.clone();
        let media_dir = media_dir.clone();
        async move {
            let result =
                download_image(&client, &url, &media_dir, &format!("ss_{idx}"), deadline).await;
            (idx, url, result)
        }
    };

    while next < need_download.len() && in_flight.len() < DOWNLOAD_CONCURRENCY {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        let (idx, url) = need_download[next].clone();
        next += 1;
        in_flight.push(spawn_one(idx, url));
    }

    while let Some((idx, url, result)) = in_flight.next().await {
        if let Some((path, resolved)) = result {
            // Prefer the final CDN URL when the download redirected.
            let _ = resolved;
            let _ = url;
            paths[idx] = Some(path);
        }
        if tokio::time::Instant::now() < deadline && next < need_download.len() {
            let (idx, url) = need_download[next].clone();
            next += 1;
            in_flight.push(spawn_one(idx, url));
        }
    }

    let downloaded = paths.iter().filter(|p| p.is_some()).count();
    if downloaded == 0 {
        tracing::warn!(game_id, thread_id, "screenshot cache downloaded 0 images");
        // Still register stubs so the gallery lists every F95 URL.
    }

    // Drop orphan files that are no longer in the gallery set.
    let keep: std::collections::HashSet<&str> = paths
        .iter()
        .filter_map(|p| p.as_deref())
        .collect();
    if let Ok(existing) = db.list_game_media(game_id) {
        for m in existing.into_iter().filter(|m| m.media_type == "screenshot") {
            if let Some(path) = m.local_path.as_deref().filter(|p| !p.trim().is_empty()) {
                if !keep.contains(path) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    db.clear_game_screenshot_media(game_id)?;
    for (i, url) in gallery.iter().enumerate() {
        let local = paths[i].as_deref().unwrap_or("");
        if let Err(e) = db.insert_media(game_id, url, local, "screenshot") {
            tracing::warn!(error = %e, "failed to record screenshot media");
        }
    }

    tracing::info!(
        game_id,
        thread_id,
        cached = downloaded,
        total = gallery.len(),
        "screenshot gallery cache complete"
    );
    Ok(downloaded)
}

pub async fn cache_thread_media(
    db: &Database,
    client: &F95Client,
    game_id: i64,
    thread_id: i64,
    cover_url: &str,
    screenshots: &[String],
) -> AppResult<Option<String>> {
    // Synchronous full cache (cover + gallery). Prefer cover-fast + background
    // screenshots on add/refresh so proxies don't 502.
    const MEDIA_BUDGET: Duration = Duration::from_secs(12);
    const MAX_SCREENSHOTS: usize = 8;

    let deadline = tokio::time::Instant::now() + MEDIA_BUDGET;

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

    if !effective_cover.is_empty() && tokio::time::Instant::now() < deadline {
        if let Some((path, resolved)) =
            download_image(client, &effective_cover, &media_dir, "cover", deadline).await
        {
            stored_cover_url = resolved;
            if let Err(e) = db.insert_media(game_id, &stored_cover_url, &path, "cover") {
                tracing::warn!(error = %e, "failed to record cover media");
            } else {
                cover_path = Some(path);
            }
        }
    }

    let mut ss_index = 0;
    for url in upgraded_screenshots
        .iter()
        .filter(|u| !u.is_empty() && !text::is_branding_image(u))
    {
        if ss_index >= MAX_SCREENSHOTS || tokio::time::Instant::now() >= deadline {
            break;
        }
        if !stored_cover_url.is_empty()
            && text::upgrade_image_url(url) == text::upgrade_image_url(&stored_cover_url)
        {
            continue;
        }
        if let Some((path, resolved)) =
            download_image(client, url, &media_dir, &format!("ss_{ss_index}"), deadline).await
        {
            if let Err(e) = db.insert_media(game_id, &resolved, &path, "screenshot") {
                tracing::warn!(error = %e, "failed to record screenshot media");
                continue;
            }
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
    deadline: tokio::time::Instant,
) -> Option<(String, String)> {
    if url.is_empty() || tokio::time::Instant::now() >= deadline {
        return None;
    }

    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        return None;
    }
    // Cap each image attempt so one slow CDN URL cannot burn the whole budget.
    let per_image = remaining.min(Duration::from_secs(8));

    match tokio::time::timeout(per_image, download_image_inner(client, url, dir, basename)).await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            tracing::debug!(%url, error = %e, "F95 image download failed");
            None
        }
        Err(_) => {
            tracing::debug!(%url, "F95 image download timed out");
            None
        }
    }
}

async fn download_image_inner(
    client: &F95Client,
    url: &str,
    dir: &Path,
    basename: &str,
) -> AppResult<Option<(String, String)>> {
    let resolved = resolve_download_url(client, url).await;

    let response = client
        .client
        .get(&resolved)
        .header("Referer", "https://f95zone.to/")
        .timeout(Duration::from_secs(8))
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
        let Ok(Ok(response)) = tokio::time::timeout(
            Duration::from_secs(6),
            client.client.get(&target).timeout(Duration::from_secs(6)).send(),
        )
        .await
        else {
            continue;
        };
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
    // Always emit forward slashes — Windows Path::display() uses `\`, which breaks URL clients.
    let rel = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if rel.is_empty() {
        return None;
    }
    Some(format!("/api/v1/media/{rel}"))
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
            tokio::time::Instant::now() + Duration::from_secs(30),
        )
        .await
        .expect("should download");
        let (path, url) = result;
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
    use super::{
        build_catalog_list_url, extract_creator, extract_download_links, extract_platforms,
        normalize_creator, parse_f95_thread_id, parse_f95_thread_slug, parse_list_page,
        parse_list_response, parse_thread_html, CatalogFilter,
    };

    #[test]
    fn download_links_skip_css_and_keep_hosters() {
        let html = r#"
            <b>Windows</b>
            <a href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.0.0/css/all.min.css">css</a>
            <a href="https://gofile.io/d/abc123">gofile</a>
            <a href="https://mega.nz/file/xyz">mega</a>
            <a href="/styles/next/css/fonts.css">local css</a>
            <a href="https://datanodes.to/abc/game-win.zip">datanodes</a>
            <a href="https://f95zone.to/masked/pixeldrain.com/1/2/abc">masked pd</a>
            <a href="https://example.com/game.zip">zip</a>
            <a href="https://example.com/page.html">page</a>
            <b>Linux</b>
            <a href="https://datanodes.to/xyz/game-linux.zip">linux</a>
        "#;
        let links = extract_download_links(html);
        let hosts: Vec<_> = links.iter().map(|l| l.host.as_str()).collect();
        assert!(hosts.contains(&"gofile"));
        assert!(hosts.contains(&"mega"));
        assert!(hosts.contains(&"datanodes"));
        assert!(hosts.contains(&"pixeldrain"));
        assert!(hosts.contains(&"http")); // zip
        assert!(!links.iter().any(|l| l.url.contains("all.min.css")));
        assert!(!links.iter().any(|l| l.url.contains("page.html")));
        let win = links
            .iter()
            .find(|l| l.url.contains("game-win.zip"))
            .expect("win zip");
        assert_eq!(win.label.as_deref(), Some("Windows"));
        let linux = links
            .iter()
            .find(|l| l.url.contains("game-linux.zip"))
            .expect("linux zip");
        assert_eq!(linux.label.as_deref(), Some("Linux"));
    }

    #[test]
    fn download_links_label_pc_and_windows_linux() {
        let html = r#"
            <b>PC</b>
            <a href="https://gofile.io/d/renpy-both">both</a>
            <b>Windows / Linux</b>
            <a href="https://pixeldrain.com/u/dualpack">dual</a>
            <b>Linux</b>
            <a href="https://datanodes.to/xyz/game-linux-only.zip">linuxonly</a>
        "#;
        let links = extract_download_links(html);
        let pc = links
            .iter()
            .find(|l| l.url.contains("renpy-both"))
            .expect("pc link");
        assert_eq!(pc.label.as_deref(), Some("PC"));
        let dual = links
            .iter()
            .find(|l| l.url.contains("dualpack"))
            .expect("dual link");
        assert_eq!(dual.label.as_deref(), Some("Windows/Linux"));
        let linux = links
            .iter()
            .find(|l| l.url.contains("game-linux-only.zip"))
            .expect("linux only");
        assert_eq!(linux.label.as_deref(), Some("Linux"));
    }

    #[test]
    fn download_links_capture_section_titles() {
        let html = r#"
<div class="bbWrapper">
  <b>DOWNLOAD</b><br>
  <b>Episode 5 - Full</b><br>
  <b>Windows</b><br>
  <a href="https://gofile.io/d/ep5-win">gofile</a>
  <a href="https://mega.nz/file/ep5win">mega</a>
  <b>Linux</b><br>
  <a href="https://datanodes.to/ep5-linux.zip">linux</a>
  <br>
  <b>Episode 4 (Hotfix)</b><br>
  <b>Windows / Linux</b><br>
  <a href="https://pixeldrain.com/u/ep4-dual">dual</a>
  <br>
  <strong>v0.3 Legacy - Android</strong><br>
  <a href="https://mediafire.com/file/ep3.apk">apk</a>
</div>
"#;
        let links = extract_download_links(html);
        let ep5_win = links
            .iter()
            .find(|l| l.url.contains("ep5-win"))
            .expect("ep5 win");
        assert_eq!(ep5_win.title.as_deref(), Some("Episode 5 - Full"));
        assert_eq!(ep5_win.label.as_deref(), Some("Windows"));

        let ep5_linux = links
            .iter()
            .find(|l| l.url.contains("ep5-linux"))
            .expect("ep5 linux");
        assert_eq!(ep5_linux.title.as_deref(), Some("Episode 5 - Full"));
        assert_eq!(ep5_linux.label.as_deref(), Some("Linux"));

        let ep4 = links
            .iter()
            .find(|l| l.url.contains("ep4-dual"))
            .expect("ep4");
        assert_eq!(ep4.title.as_deref(), Some("Episode 4 (Hotfix)"));
        assert_eq!(ep4.label.as_deref(), Some("Windows/Linux"));

        let apk = links
            .iter()
            .find(|l| l.url.contains("ep3.apk"))
            .expect("apk");
        assert_eq!(apk.title.as_deref(), Some("v0.3 Legacy"));
        assert_eq!(apk.label.as_deref(), Some("Android"));
    }

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
        assert_eq!(
            parse_f95_thread_slug(
                "https://f95zone.to/threads/the-seven-realms-r4-v1-06-septcloudgames.112700/"
            )
            .as_deref(),
            Some("the-seven-realms-r4-v1-06-septcloudgames")
        );
        assert_eq!(parse_f95_thread_slug("112700"), None);
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

    #[test]
    fn extracts_img_src_and_keeps_more_than_sixteen_images() {
        let mut imgs = String::new();
        for i in 0..25 {
            imgs.push_str(&format!(
                r#"<img src="https://attachments.f95zone.to/2024/01/shot_{i}.png" />"#
            ));
        }
        let html = format!(
            r#"
<article class="message-threadStarterPost">
  <div class="message-body">
    <div class="bbWrapper">{imgs}</div>
  </div>
</article>
"#
        );
        let meta = parse_thread_html(1, &html).unwrap();
        assert!(
            meta.screenshots.len() >= 24,
            "expected full gallery, got {}",
            meta.screenshots.len()
        );
        assert!(
            meta.screenshots.iter().any(|u| u.contains("shot_24")),
            "missing last screenshot"
        );
    }

    #[test]
    fn extracts_gif_urls_from_first_post() {
        let html = r#"
<article class="message-threadStarterPost">
  <div class="message-body">
    <div class="bbWrapper">
      <a class="js-lbImage" href="https://attachments.f95zone.to/2024/01/demo.gif">
        <img data-src="https://attachments.f95zone.to/2024/01/demo.gif" />
      </a>
      <img src="https://attachments.f95zone.to/2024/01/still.webp" />
    </div>
  </div>
</article>
"#;
        let meta = parse_thread_html(1, html).unwrap();
        assert!(
            meta.all_images.iter().any(|u| u.to_lowercase().contains(".gif")),
            "gif missing: {:?}",
            meta.all_images
        );
        assert!(
            meta.all_images.iter().any(|u| u.to_lowercase().contains(".webp")),
            "webp missing: {:?}",
            meta.all_images
        );
    }

    #[test]
    fn extracts_rating_from_ld_json() {
        let html = r#"
        <html><head>
        <script type="application/ld+json">
        {"@type":"DiscussionForumPosting","aggregateRating":{"@type":"AggregateRating","ratingValue":"4.12","ratingCount":"128"}}
        </script>
        </head><body><h1>Village Slut Transformations</h1></body></html>
        "#;
        let meta = parse_thread_html(14060, html).unwrap();
        assert!((meta.result.rating - 4.12).abs() < 0.001);
    }

    #[test]
    fn parse_ignores_reply_posts_after_op() {
        let html = r#"
<html><head><meta property="og:title" content="Only OP Game"></head>
<body>
  <h1 class="p-title-value">Only OP Game</h1>
  <article class="message message-threadStarterPost" data-author="DevOne">
    <div class="bbWrapper">
      <b>Developer</b>: DevOne<br>
      <b>Version</b>: 1.0<br>
      <b>OS</b>: Windows<br>
      <b>Overview:</b><br>
      Real overview text for the game that is long enough to keep.
      <a href="https://gofile.io/d/op-only">gofile</a>
    </div>
  </article>
  <article class="message" data-author="RandomReply">
    <div class="bbWrapper">
      <b>OS</b>: Android<br>
      <b>Overview:</b><br>
      This reply must never be scraped as game metadata.
      <a href="https://mega.nz/file/reply-should-skip">mega</a>
    </div>
  </article>
</body></html>
"#;
        let meta = parse_thread_html(42, html).expect("parse");
        assert_eq!(meta.result.title, "Only OP Game");
        assert!(
            meta.description
                .as_deref()
                .unwrap_or("")
                .contains("Real overview"),
            "expected OP overview, got {:?}",
            meta.description
        );
        assert!(
            !meta
                .description
                .as_deref()
                .unwrap_or("")
                .contains("never be scraped"),
            "reply leaked into description"
        );
        assert_eq!(meta.result.platforms, vec!["Windows".to_string()]);
        assert!(!meta.result.platforms.iter().any(|p| p == "Android"));

        let links = extract_download_links(html);
        assert!(links.iter().any(|l| l.url.contains("op-only")));
        assert!(
            links.iter().all(|l| !l.url.contains("reply-should-skip")),
            "reply hoster link leaked: {links:?}"
        );
    }

    #[test]
    fn parses_sam_rating_number() {
        let json = r#"{"status":"ok","msg":{"data":[{"thread_id":1,"title":"Test","rating":4.82}]}}"#;
        let results = parse_list_response(json).unwrap();
        assert!((results[0].rating - 4.82).abs() < 0.001);
    }

    #[test]
    fn parses_sam_rating_string() {
        let json = r#"{"status":"ok","msg":{"data":[{"thread_id":1,"title":"Test","rating":"4.5"}]}}"#;
        let results = parse_list_response(json).unwrap();
        assert!((results[0].rating - 4.5).abs() < 0.001);
    }

    #[test]
    fn parses_sam_pagination_totals() {
        let json = r#"{"status":"ok","msg":{"data":[{"thread_id":1,"title":"Test"}],"pagination":{"page":2,"total":17}}}"#;
        let page = parse_list_page(json, 1, 90).unwrap();
        assert_eq!(page.page, 2);
        assert_eq!(page.total_pages, 17);
        assert!(page.has_more());
        assert_eq!(page.items.len(), 1);

        let last = r#"{"status":"ok","msg":{"data":[{"thread_id":2,"title":"Last"}],"pagination":{"page":17,"total":17}}}"#;
        let page = parse_list_page(last, 1, 90).unwrap();
        assert!(!page.has_more());
    }

    #[test]
    fn extracts_developer_from_overview_not_user_banner() {
        let html = r#"
<article data-author="Mr Fable" class="message">
  <div class="userBanner message-userBanner" itemprop="jobTitle"><strong>Game Developer</strong></div>
  <h5>Creator of &quot;Whispers of Desire&quot;</h5>
  <div class="bbWrapper">
    <b>Developer</b>: Mr_Fable - Steam - Patreon - SubscribeStar<br />
    <b>Version</b>: 0.5
  </div>
</article>
"#;
        assert_eq!(extract_creator(html), "Mr_Fable");
    }

    #[test]
    fn overview_parse_survives_unicode_before_marker() {
        // ß → "ss" under Unicode to_lowercase; finding in lower and slicing original used to panic.
        let html = r#"
<article class="message message-threadStarterPost">
  <h1 class="p-title-value">The Seven Realms</h1>
  <div class="bbWrapper">
    Dev: Straße Café<br>
    Overview: A long enough fantasy overview for The Seven Realms with enough characters to keep.
    Thread Updated: 2024-01-01
  </div>
</article>
"#;
        let meta = parse_thread_html(999001, html).expect("parse must not panic");
        assert!(meta.result.title.to_lowercase().contains("seven"));
        let desc = meta.description.expect("overview description");
        assert!(desc.to_lowercase().contains("fantasy") || desc.to_lowercase().contains("seven"));
    }

    #[test]
    fn extracts_developer_plain_text_overview() {
        let html = r#"
<div class="bbWrapper">
Developer: Paper Tiger Discord - SubscribeStar - Patreon<br>
Version: 0.12
</div>
"#;
        assert_eq!(extract_creator(html), "Paper Tiger");
    }

    #[test]
    fn extracts_platforms_from_overview_variants() {
        let html = r#"
<div class="bbWrapper">
<b>Developer</b>: DevName<br />
<b>OS</b>: Windows / OSX / Andriod<br />
<b>Version</b>: 0.5
</div>
"#;
        assert_eq!(
            extract_platforms(html),
            vec!["Windows", "Mac", "Android"]
        );

        let html2 = r#"
<div class="bbWrapper">
Platform: PC, Mac OS X, Linux
</div>
"#;
        assert_eq!(
            extract_platforms(html2),
            vec!["Windows", "Mac", "Linux"]
        );

        let meta = parse_thread_html(
            1,
            r#"
<article class="message message-threadStarterPost">
  <div class="bbWrapper">
    <b>OS</b>: Window / Android<br />
  </div>
</article>
"#,
        )
        .unwrap();
        assert_eq!(meta.result.platforms, vec!["Windows", "Android"]);
    }

    #[test]
    fn extract_platforms_utf8_does_not_panic() {
        // Mid-codepoint window cuts used to panic (`byte index is not a char boundary`),
        // which aborted the HTTP handler → immediate Cloudflare 502 with no JSON body.
        let mut html = String::from("<div><b>OS</b>: Windows / Linux<br />");
        for _ in 0..200 {
            html.push('日'); // 3-byte UTF-8 so idx+320 can land mid-character
        }
        html.push_str("</div>");
        let platforms = extract_platforms(&html);
        assert_eq!(platforms, vec!["Windows", "Linux"]);

        // Unicode to_lowercase expands ß→ss and used to mis-index into the original HTML.
        let mut html2 = String::new();
        for _ in 0..80 {
            html2.push('ß');
        }
        html2.push_str(" OS: Android<br />");
        for _ in 0..80 {
            html2.push('日');
        }
        let _ = extract_platforms(&html2); // must not panic
        let _ = extract_platforms(&html); // second pass still safe
    }

    #[test]
    fn normalize_creator_rejects_unknown() {
        assert_eq!(normalize_creator("Unknown"), None);
        assert_eq!(
            normalize_creator("  Mr_Fable - Patreon "),
            Some("Mr_Fable".into())
        );
    }

    #[test]
    fn catalog_list_url_strips_apostrophes_in_search() {
        let url = build_catalog_list_url(&CatalogFilter {
            search: "Angel's Love".into(),
            ..CatalogFilter::default()
        })
        .unwrap();
        assert!(url.contains("search=Angels%20Love") || url.contains("search=Angels+Love"), "{url}");
        assert!(!url.contains("Angel%27s"), "{url}");
    }

    #[test]
    fn catalog_list_url_resolves_names_and_keeps_sort() {
        let url = build_catalog_list_url(&CatalogFilter {
            sort: "likes".into(),
            page: 2,
            tags: vec!["female protagonist".into()],
            tag_mode: "and".into(),
            ..CatalogFilter::default()
        })
        .unwrap();
        assert!(url.contains("sort=likes"), "{url}");
        assert!(url.contains("page=2"), "{url}");
        assert!(
            url.contains("tags%5B%5D=392") || url.contains("tags[]=392"),
            "{url}"
        );
        assert!(!url.contains("tagtype=or"), "{url}");
        assert!(!url.to_lowercase().contains("female"), "{url}");
    }

    #[test]
    fn catalog_list_url_passes_pre_resolved_ids_with_or_mode() {
        let url = build_catalog_list_url(&CatalogFilter {
            sort: "rating".into(),
            tags: vec!["392".into(), "783".into()],
            tag_mode: "or".into(),
            date_days: 30,
            rows: 90,
            ..CatalogFilter::default()
        })
        .unwrap();
        assert!(url.contains("sort=rating"), "{url}");
        assert!(url.contains("tagtype=or"), "{url}");
        assert!(url.contains("date=30"), "{url}");
        assert!(url.contains("rows=90"), "{url}");
        assert!(
            url.contains("tags%5B%5D=392") || url.contains("tags[]=392"),
            "{url}"
        );
        assert!(
            url.contains("tags%5B%5D=783") || url.contains("tags[]=783"),
            "{url}"
        );
    }

    #[test]
    fn catalog_list_url_rejects_unknown_tag_names() {
        let err = build_catalog_list_url(&CatalogFilter {
            tags: vec!["definitely-not-a-real-f95-tag-xyz".into()],
            ..CatalogFilter::default()
        })
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unknown F95"), "{msg}");
    }

    #[test]
    fn reqwest_url_parse_keeps_tag_ids() {
        let raw = build_catalog_list_url(&CatalogFilter {
            tags: vec!["392".into()],
            sort: "likes".into(),
            rows: 90,
            ..CatalogFilter::default()
        })
        .unwrap();
        let parsed = reqwest::Url::parse(&raw).expect("url parse");
        let q = parsed.query().unwrap_or("");
        assert!(q.contains("392"), "query={q}");
        assert!(
            q.contains("tags%5B%5D=392") || q.contains("tags[]=392"),
            "query={q}"
        );
        assert!(q.contains("sort=likes"), "query={q}");
    }
}
