use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::ScanResult;
use std::path::Path;
use walkdir::WalkDir;

const ARCHIVE_EXTENSIONS: &[&str] = &["bz2", "rar", "zip", "7z"];

const NOISE_WORDS: &[&str] = &[
    "final", "complete", "premium", "edition", "repack", "repacks", "uncensored", "censored",
    "patched", "patch", "update", "updated", "patreon", "steam", "gog", "build", "release",
    "compressed", "archive", "game", "win", "win64", "win32", "linux", "mac", "android",
    "x64", "x86", "apk", "mod", "mods", "dlc", "bonus", "pack", "official", "english",
    "eng", "rus", "multi", "standalone", "installer", "portable", "fixed", "fix",
];

const VERSION_PREFIXES: &[&str] = &["v", "ver", "version", "ch", "chapter", "ep", "episode", "season", "s"];

pub fn scan_archive_folder(db: &Database, archive_path: &str) -> AppResult<ScanResult> {
    let path = Path::new(archive_path);
    if !path.exists() {
        return Err(AppError::BadRequest(format!(
            "archive path does not exist: {archive_path}"
        )));
    }
    if !path.is_dir() {
        return Err(AppError::BadRequest(format!(
            "archive path is not a directory: {archive_path}"
        )));
    }

    let mut added = 0usize;
    let mut updated = 0usize;
    let mut total = 0usize;

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let file_path = entry.path();
        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !ARCHIVE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            continue;
        }

        total += 1;
        let metadata = std::fs::metadata(file_path)?;
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let full_path = file_path.display().to_string();
        let (is_new, _) = db.upsert_archive(&full_path, &filename, metadata.len() as i64)?;
        if is_new {
            added += 1;
        } else {
            updated += 1;
        }
    }

    Ok(ScanResult {
        added,
        updated,
        total,
    })
}

/// Strip archive extensions like .tar.bz2
fn strip_archive_name(filename: &str) -> &str {
    let mut name = filename;
    loop {
        if let Some((stem, ext)) = name.rsplit_once('.') {
            let ext_lower = ext.to_lowercase();
            if ARCHIVE_EXTENSIONS.contains(&ext_lower.as_str()) || ext_lower == "tar" {
                name = stem;
                continue;
            }
        }
        break;
    }
    name
}

fn is_version_token(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return true;
    }
    for prefix in VERSION_PREFIXES {
        if lower.starts_with(prefix) {
            let rest = &lower[prefix.len()..];
            if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return true;
            }
        }
    }
    false
}

fn is_noise_word(word: &str) -> bool {
    NOISE_WORDS.contains(&word.to_lowercase().as_str())
}

fn tokenize_name(name: &str) -> Vec<String> {
    name.split(|c: char| c == '_' || c == '-' || c == '.' || c == '+' || c == ' ')
        .filter(|w| !w.is_empty())
        .map(|w| w.trim().to_string())
        .collect()
}

fn words_to_title(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| {
            if w.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
                w.to_string()
            } else if w.len() <= 3 && w.chars().all(|c| c.is_ascii_alphabetic()) {
                // Keep short words like "a", "of", or acronyms like "DIK"
                if w.chars().all(|c| c.is_ascii_uppercase()) {
                    w.to_string()
                } else {
                    w.to_string()
                }
            } else {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn clean_words(words: &[String]) -> Vec<String> {
    words
        .iter()
        .filter(|w| !is_noise_word(w) && !is_version_token(w))
        .cloned()
        .collect()
}

/// Generate multiple search queries from an archive filename, best first.
pub fn guess_search_queries(filename: &str) -> Vec<String> {
    let base = strip_archive_name(filename);
    let raw_tokens = tokenize_name(base);
    let cleaned: Vec<String> = clean_words(&raw_tokens);

    let mut queries = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let push_query = |queries: &mut Vec<String>, seen: &mut std::collections::HashSet<String>, words: &[String]| {
        if words.is_empty() {
            return;
        }
        let refs: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
        let title = words_to_title(&refs);
        if title.len() >= 2 && seen.insert(title.to_lowercase()) {
            queries.push(title);
        }
    };

    // Full cleaned name
    push_query(&mut queries, &mut seen, &cleaned);

    // Progressively drop trailing tokens (usually version/build info)
    for end in (1..cleaned.len()).rev() {
        push_query(&mut queries, &mut seen, &cleaned[..end]);
    }

    // Drop leading noise (e.g. "[TeamXYZ] Game Name")
    if cleaned.len() > 2 {
        for start in 1..cleaned.len().saturating_sub(1) {
            push_query(&mut queries, &mut seen, &cleaned[start..]);
        }
    }

    // Space-separated version of raw name without version-like tail
    let raw_cleaned = clean_words(&raw_tokens);
    if raw_cleaned != cleaned {
        push_query(&mut queries, &mut seen, &raw_cleaned);
    }

    queries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versioned_name() {
        let queries = guess_search_queries("Being_a_DIK_v0.12.0.zip");
        assert!(queries.iter().any(|q| q.contains("DIK")));
        assert!(!queries.iter().any(|q| q.contains("0.12")));
    }

    #[test]
    fn parses_chapter_name() {
        let queries = guess_search_queries("Depraved_Awakening_Ch.2_v2.17_Patreon.zip");
        assert!(queries.iter().any(|q| q.to_lowercase().contains("depraved")));
    }
}
