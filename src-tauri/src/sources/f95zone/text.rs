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
}
