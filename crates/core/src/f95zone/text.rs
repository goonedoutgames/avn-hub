//! HTML entity decoding and F95 title/cover normalization.

pub fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            for ch in chars.by_ref() {
                if ch == ';' {
                    break;
                }
                entity.push(ch);
            }
            let decoded = match entity.as_str() {
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                "apos" | "#39" | "#039" | "#x27" | "#X27" => "'",
                "rsquo" | "#8217" | "#x2019" | "#X2019" => "'",
                "lsquo" | "#8216" | "#x2018" | "#X2018" => "'",
                s if s.starts_with("#x") || s.starts_with("#X") => {
                    if let Ok(code) = u32::from_str_radix(&s[2..], 16) {
                        if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                            continue;
                        }
                    }
                    "&"
                }
                s if s.starts_with('#') => {
                    if let Ok(code) = s[1..].parse::<u32>() {
                        if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                            continue;
                        }
                    }
                    "&"
                }
                _ => "&",
            };
            out.push_str(decoded);
        } else {
            out.push(c);
        }
    }
    out
}

/// Normalize curly/smart quotes to ASCII apostrophe.
pub fn normalize_apostrophes(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '\u{2019}' | '\u{2018}' | '\u{00B4}' | '\u{0060}' => '\'',
            _ => c,
        })
        .collect()
}

/// Remove apostrophes for F95 search (e.g. "Angel's" → "Angels").
pub fn strip_apostrophes_for_search(input: &str) -> String {
    normalize_apostrophes(input)
        .chars()
        .filter(|c| *c != '\'')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const F95_PREFIXES: &[&str] = &[
    "vn",
    "ren'py",
    "renpy",
    "unity",
    "rpgm",
    "html",
    "qsp",
    "unreal engine",
    "flash",
    "java",
    "other",
    "completed",
    "abandoned",
    "on hold",
    "cancelled",
];

/// Engine / status prefixes from an F95 title (before cleaning).
pub fn extract_title_prefixes(raw: &str) -> Vec<String> {
    let title = normalize_apostrophes(&decode_html_entities(raw).trim().to_string());
    let mut title = title;
    if let Some(idx) = title.to_lowercase().find("| f95") {
        title = title[..idx].trim().to_string();
    }

    let parts: Vec<&str> = title.split(" - ").map(str::trim).collect();
    if parts.len() <= 1 {
        return Vec::new();
    }

    let mut prefixes = Vec::new();
    for part in &parts[..parts.len() - 1] {
        let lower = part.to_lowercase();
        if F95_PREFIXES.iter().any(|p| lower == *p) {
            // Prefer canonical casing from known list / common names
            let label = match lower.as_str() {
                "renpy" | "ren'py" => "Ren'Py".into(),
                "vn" => "VN".into(),
                "unity" => "Unity".into(),
                "html" => "HTML".into(),
                "rpgm" => "RPGM".into(),
                "completed" => "Completed".into(),
                "abandoned" => "Abandoned".into(),
                "on hold" => "On Hold".into(),
                "cancelled" => "Cancelled".into(),
                other => {
                    let mut c = other.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => other.to_string(),
                    }
                }
            };
            if !prefixes.iter().any(|p: &String| p.eq_ignore_ascii_case(&label)) {
                prefixes.push(label);
            }
        } else {
            break;
        }
    }
    prefixes
}

/// Strip F95 category/status prefixes from thread or og:title strings.
pub fn clean_f95_title(raw: &str) -> String {
    let mut title = normalize_apostrophes(&decode_html_entities(raw).trim().to_string());

    if let Some(idx) = title.to_lowercase().find("| f95") {
        title = title[..idx].trim().to_string();
    }

    let parts: Vec<&str> = title.split(" - ").map(str::trim).collect();
    if parts.len() <= 1 {
        return title;
    }

    let mut start = 0usize;
    while start < parts.len().saturating_sub(1) {
        let lower = parts[start].to_lowercase();
        if F95_PREFIXES.iter().any(|p| lower == *p) {
            start += 1;
        } else {
            break;
        }
    }

    parts[start..].join(" - ")
}

pub fn is_branding_image(url: &str) -> bool {
    let lower = url.to_lowercase();
    // Game banners/screenshots on the F95 attachment CDN are valid media.
    if lower.contains("attachments.f95zone.to") || lower.contains("preview.f95zone.to") {
        return false;
    }
    lower.contains("f95zone.to/styles")
        || lower.contains("/styles/")
        || lower.contains("data/assets/logo")
        || lower.contains("/logo")
        || lower.contains("favicon")
        || lower.contains("xenforo")
        || lower.ends_with("/og.png")
        || lower.contains("og-image")
        || (lower.contains("f95zone") && lower.contains("banner") && !lower.contains("attachments"))
}

pub fn is_post_banner(url: &str) -> bool {
    let lower = upgrade_image_url(url).to_lowercase();
    lower.contains("banner")
}

/// Split the first-post image list into a cover (banner/first image) and screenshots.
pub fn split_cover_and_screenshots(images: &[String]) -> (String, Vec<String>) {
    let upgraded: Vec<String> = images
        .iter()
        .map(|u| upgrade_image_url(u))
        .filter(|u| !u.is_empty() && !is_branding_image(u))
        .collect();

    if upgraded.is_empty() {
        return (String::new(), Vec::new());
    }

    let banner_idx = upgraded.iter().position(|u| is_post_banner(u));
    let cover = banner_idx
        .map(|i| upgraded[i].clone())
        .unwrap_or_else(|| upgraded[0].clone());

    let screenshots: Vec<String> = upgraded
        .into_iter()
        .enumerate()
        .filter(|(i, _)| banner_idx != Some(*i))
        .map(|(_, u)| u)
        .collect();

    (cover, screenshots)
}

pub fn pick_best_cover(cover: &str, screenshots: &[String]) -> String {
    let cover = upgrade_image_url(cover.trim());
    if !cover.is_empty() && !is_branding_image(&cover) && is_post_banner(&cover) {
        return cover;
    }
    if let Some(banner) = screenshots.iter().find(|u| is_post_banner(u)) {
        let banner = upgrade_image_url(banner);
        if !banner.is_empty() && !is_branding_image(&banner) {
            return banner;
        }
    }
    if !cover.is_empty() && !is_branding_image(&cover) {
        return cover;
    }
    screenshots
        .iter()
        .map(|url| upgrade_image_url(url))
        .find(|url| !url.is_empty() && !is_branding_image(url))
        .unwrap_or_default()
}

/// Map F95 preview CDN URLs to full-resolution attachment CDN URLs.
pub fn full_attachment_url(url: &str) -> String {
    let u = url.trim().to_string();
    if u.is_empty() {
        return u;
    }
    let lower = u.to_lowercase();
    if lower.contains("preview.f95zone.to") {
        return u.replacen("preview.f95zone.to", "attachments.f95zone.to", 1)
            .replacen("PREVIEW.F95ZONE.TO", "attachments.f95zone.to", 1);
    }
    u
}

/// Upgrade thumbnail/proxy URLs to their full-resolution attachment URL.
pub fn upgrade_image_url(url: &str) -> String {
    let mut u = url.trim().to_string();
    if u.is_empty() {
        return u;
    }

    u = u.replace(".thumb.", ".");
    u = u.replace("/thumb/", "/");

    for ext in [".png", ".jpg", ".jpeg", ".webp", ".gif"] {
        let suffix = format!("-thumb{ext}");
        if let Some(idx) = u.to_lowercase().rfind(&suffix) {
            u = format!("{}{}", &u[..idx], ext);
        }
        let thumb_suffix = format!("-thumbnail{ext}");
        if let Some(idx) = u.to_lowercase().rfind(&thumb_suffix) {
            u = format!("{}{}", &u[..idx], ext);
        }
    }

    if u.contains('?') {
        let lower = u.to_lowercase();
        if lower.contains("width=")
            || lower.contains("height=")
            || lower.contains("thumb=")
            || lower.contains("/thumb")
        {
            u = u.split_once('?').map(|(base, _)| base).unwrap_or(&u).to_string();
        }
    }

    full_attachment_url(&u)
}

pub fn looks_like_tag_ids(tags: &[String]) -> bool {
    !tags.is_empty() && tags.iter().all(|t| t.chars().all(|c| c.is_ascii_digit()))
}

/// Canonical platform labels used in the library / browse UI.
pub const CANONICAL_PLATFORMS: &[&str] = &[
    "Windows", "Mac", "Linux", "Android", "iOS", "Web",
];

/// Parse a free-form OS / platform field into canonical labels.
///
/// Accepts typos and aliases common on F95 first posts, e.g.
/// `Windows / OSX / Andriod`, `PC, MacOS`, `Win + Linux`.
pub fn parse_platforms(raw: &str) -> Vec<String> {
    let decoded = decode_html_entities(raw);
    let normalized = decoded
        .replace('\u{00a0}', " ")
        .replace(['|', ';', '+', '&'], ",")
        .replace(" / ", ",")
        .replace('/', ",");

    let mut out = Vec::new();
    for part in normalized.split([',', '\n']) {
        let token = part.trim().trim_matches(|c: char| {
            c == '-' || c == '–' || c == '—' || c == ':' || c == '.'
        });
        if token.is_empty() {
            continue;
        }
        // Split glued phrases like "Windows Mac Android" when no separators.
        for piece in split_platform_tokens(token) {
            if let Some(label) = canonicalize_platform(&piece) {
                if !out.iter().any(|p: &String| p.eq_ignore_ascii_case(&label)) {
                    out.push(label);
                }
            }
        }
    }
    out
}

fn split_platform_tokens(token: &str) -> Vec<String> {
    let lower = token.to_lowercase();
    // Multi-word aliases that should stay intact.
    const MULTI: &[&str] = &[
        "mac os x",
        "mac osx",
        "mac os",
        "os x",
        "os-x",
        "operating system",
    ];
    for m in MULTI {
        if lower == *m {
            return vec![token.to_string()];
        }
    }

    // If the whole token already maps, keep it.
    if canonicalize_platform(token).is_some() {
        return vec![token.to_string()];
    }

    // Split on whitespace for "Windows Android Linux".
    let words: Vec<&str> = token.split_whitespace().collect();
    if words.len() <= 1 {
        return vec![token.to_string()];
    }

    let mut pieces = Vec::new();
    let mut i = 0;
    while i < words.len() {
        // Try 3- then 2-word windows for "Mac OS X" / "Mac OS".
        let mut matched = false;
        for len in [3usize, 2].iter().copied() {
            if i + len <= words.len() {
                let joined = words[i..i + len].join(" ");
                if canonicalize_platform(&joined).is_some() {
                    pieces.push(joined);
                    i += len;
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        pieces.push(words[i].to_string());
        i += 1;
    }
    pieces
}

fn canonicalize_platform(token: &str) -> Option<String> {
    let mut key = token.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    // Collapse separators inside a single token: "os-x", "mac_os", "win64".
    key = key
        .chars()
        .map(|c| if c == '_' || c == '-' || c == '.' { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Strip trailing version crumbs: "windows 10", "android 12".
    let stripped = key
        .trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ')
        .trim()
        .to_string();
    let key = if stripped.is_empty() { key } else { stripped };

    let label = match key.as_str() {
        "windows" | "window" | "win" | "win32" | "win64" | "windoze" | "pc"
        | "microsoft windows" => "Windows",
        "mac" | "macos" | "osx" | "os x" | "macintosh" | "apple"
        | "mac os" | "mac osx" | "mac os x" => "Mac",
        "linux" | "gnu linux" | "gnu/linux" | "unix" => "Linux",
        "android" | "andriod" | "andoid" | "apk" | "android os" => "Android",
        "ios" | "iphone" | "ipad" | "ipados" | "apple ios" => "iOS",
        "web" | "browser" | "html5" | "webgl" | "online" => "Web",
        other => {
            // Fuzzy: contains a known stem (handles "windowss", "macosx").
            if other.contains("android") || other.contains("andriod") {
                "Android"
            } else if other.contains("windows") || other == "winos" {
                "Windows"
            } else if other.contains("linux") {
                "Linux"
            } else if other.contains("macos")
                || other.contains("osx")
                || other.starts_with("mac ")
                || other == "macintosh"
            {
                "Mac"
            } else if other.contains("iphone") || other.contains("ipad") || other == "ios" {
                "iOS"
            } else if other.contains("browser") || other == "html5" {
                "Web"
            } else {
                return None;
            }
        }
    };
    Some(label.into())
}

pub fn is_xenforo_thumbnail(url: &str) -> bool {
    let lower = upgrade_image_url(url).to_lowercase();
    (lower.contains("thumbnail") || lower.contains("/thumb/") || lower.contains(".thumb."))
        && !lower.contains("attachments.f95zone.to")
}

/// XenForo attachment page URL (not a direct thumbnail image).
pub fn attachment_page_url(url: &str) -> String {
    let mut u = upgrade_image_url(url.trim());
    if let Some(idx) = u.to_lowercase().rfind("/thumbnail") {
        u = u[..idx].trim_end_matches('/').to_string();
    }
    u
}

/// URLs from the F95 SAM list API — keep browser-displayable (thumbnails OK for Match UI).
pub fn sam_list_media_url(url: &str) -> Option<String> {
    let u = upgrade_image_url(url.trim());
    if u.is_empty() || is_branding_image(&u) {
        return None;
    }
    Some(u)
}

/// Resolve-friendly URL for server-side download (attachment pages OK).
pub fn download_media_url(url: &str) -> Option<String> {
    let u = upgrade_image_url(url.trim());
    if u.is_empty() || is_branding_image(&u) {
        return None;
    }
    if u.contains("/attachments/") && !is_cdn_attachment(&u) {
        return Some(attachment_page_url(&u));
    }
    if is_xenforo_thumbnail(&u) {
        return None;
    }
    Some(u)
}

pub fn is_cdn_attachment(url: &str) -> bool {
    upgrade_image_url(url)
        .to_lowercase()
        .contains("attachments.f95zone.to/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_html_entities("Ren&#039;Py"), "Ren'Py");
        assert_eq!(decode_html_entities("Angel&rsquo;s Love"), "Angel's Love");
        assert_eq!(decode_html_entities("Angel&#8217;s Love"), "Angel's Love");
    }

    #[test]
    fn strips_apostrophes_for_search() {
        assert_eq!(strip_apostrophes_for_search("Angel's Love"), "Angels Love");
        assert_eq!(
            strip_apostrophes_for_search("Ren'Py - Summertime Saga"),
            "RenPy - Summertime Saga"
        );
    }

    #[test]
    fn upgrades_thumb_urls() {
        assert_eq!(
            upgrade_image_url("https://attachments.f95zone.to/foo/thumb/bar.png?width=200"),
            "https://attachments.f95zone.to/foo/bar.png"
        );
        assert_eq!(
            upgrade_image_url("https://example.com/image.thumb.jpg"),
            "https://example.com/image.jpg"
        );
    }

    #[test]
    fn upgrades_preview_cdn_to_attachments() {
        assert_eq!(
            upgrade_image_url("https://preview.f95zone.to/2025/07/5083149_c1s16tr2.png"),
            "https://attachments.f95zone.to/2025/07/5083149_c1s16tr2.png"
        );
    }

    #[test]
    fn prefers_banner_over_gameplay_cover() {
        let cover = pick_best_cover(
            "https://attachments.f95zone.to/2025/07/gameplay.png",
            &["https://attachments.f95zone.to/2026/05/Chapter4Banner.png".into()],
        );
        assert!(cover.contains("Banner"));
    }

    #[test]
    fn parses_platform_aliases_and_typos() {
        assert_eq!(
            parse_platforms("Windows / OSX / Andriod"),
            vec!["Windows", "Mac", "Android"]
        );
        assert_eq!(
            parse_platforms("PC, MacOS, Linux"),
            vec!["Windows", "Mac", "Linux"]
        );
        assert_eq!(
            parse_platforms("Win32 + Mac OS X + iOS"),
            vec!["Windows", "Mac", "iOS"]
        );
        assert_eq!(parse_platforms("Window"), vec!["Windows"]);
        assert_eq!(parse_platforms("OS-X"), vec!["Mac"]);
        assert_eq!(parse_platforms("browser / html5"), vec!["Web"]);
        assert!(parse_platforms("Unknown Device").is_empty());
    }
}
