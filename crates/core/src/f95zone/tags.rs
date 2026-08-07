//! F95Zone Latest Updates uses **numeric tag IDs** in `tags[]` / `notags[]`.
//! Names come from the site's `latestUpdates.tags` map (selectize dropdown).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

static SEED: OnceLock<TagCatalog> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct TagCatalog {
    by_id: HashMap<i64, String>,
    by_name: HashMap<String, i64>,
}

impl TagCatalog {
    pub fn seed() -> &'static TagCatalog {
        SEED.get_or_init(|| {
            let raw: HashMap<String, String> =
                serde_json::from_str(include_str!("f95_tags.json")).unwrap_or_default();
            let mut cat = TagCatalog::default();
            for (id_s, name) in raw {
                if let Ok(id) = id_s.parse::<i64>() {
                    cat.insert(id, name);
                }
            }
            cat
        })
    }

    pub fn insert(&mut self, id: i64, name: impl Into<String>) {
        let name = name.into();
        let key = normalize_tag_name(&name);
        if key.is_empty() {
            return;
        }
        self.by_id.insert(id, name);
        self.by_name.insert(key, id);
    }

    pub fn merge_from_id_map(&mut self, map: &HashMap<i64, String>) {
        for (id, name) in map {
            self.insert(*id, name.clone());
        }
    }

    pub fn id_for_name(&self, name: &str) -> Option<i64> {
        self.by_name.get(&normalize_tag_name(name)).copied()
    }

    pub fn name_for_id(&self, id: i64) -> Option<&str> {
        self.by_id.get(&id).map(|s| s.as_str())
    }

    pub fn resolve_query_token(&self, token: &str) -> Option<i64> {
        let t = token.trim();
        if t.is_empty() {
            return None;
        }
        if t.chars().all(|c| c.is_ascii_digit()) {
            return t.parse().ok();
        }
        self.id_for_name(t)
    }

    pub fn resolve_query_list(&self, tokens: &[String]) -> Result<Vec<String>, Vec<String>> {
        let mut ids = Vec::new();
        let mut unknown = Vec::new();
        for token in tokens {
            match self.resolve_query_token(token) {
                Some(id) => {
                    let s = id.to_string();
                    if !ids.contains(&s) {
                        ids.push(s);
                    }
                }
                None => unknown.push(token.clone()),
            }
        }
        if unknown.is_empty() {
            Ok(ids)
        } else {
            Err(unknown)
        }
    }

    pub fn labels_for_ids(&self, ids: &[String]) -> Vec<String> {
        ids.iter()
            .filter_map(|raw| {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return None;
                }
                if let Ok(id) = trimmed.parse::<i64>() {
                    if let Some(name) = self.name_for_id(id) {
                        return Some(name.to_string());
                    }
                }
                // Already a name
                if !trimmed.chars().all(|c| c.is_ascii_digit()) {
                    return Some(trimmed.to_string());
                }
                None
            })
            .collect()
    }

    pub fn all_sorted(&self) -> Vec<(i64, String)> {
        let mut v: Vec<_> = self
            .by_id
            .iter()
            .map(|(id, name)| (*id, name.clone()))
            .collect();
        v.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        v
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(i64, String)> {
        let q = normalize_tag_name(query);
        let mut all = self.all_sorted();
        if q.is_empty() {
            all.truncate(limit);
            return all;
        }
        all.retain(|(_, name)| normalize_tag_name(name).contains(&q));
        all.truncate(limit);
        all
    }
}

pub fn normalize_tag_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace('&', "and")
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse `latestUpdates.tags` style maps from `cmd=options` JSON.
#[derive(Debug, Deserialize)]
struct OptionsMsg {
    #[serde(default)]
    tags: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OptionsResponse {
    status: String,
    msg: Option<OptionsMsg>,
}

pub fn parse_options_tags(json_text: &str) -> Option<HashMap<i64, String>> {
    let body: OptionsResponse = serde_json::from_str(json_text).ok()?;
    if body.status != "ok" {
        return None;
    }
    let tags = body.msg?.tags?;
    let mut out = HashMap::new();
    match tags {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let id = k.parse::<i64>().ok()?;
                let name = match v {
                    serde_json::Value::String(s) => s,
                    other => other.as_str()?.to_string(),
                };
                if !name.is_empty() {
                    out.insert(id, name);
                }
            }
        }
        _ => return None,
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_has_female_protagonist() {
        let cat = TagCatalog::seed();
        assert_eq!(cat.id_for_name("female protagonist"), Some(392));
        assert_eq!(cat.id_for_name("Female Protagonist"), Some(392));
        assert_eq!(cat.name_for_id(392), Some("female protagonist"));
        assert_eq!(cat.id_for_name("2d game"), Some(2214));
        assert_eq!(cat.id_for_name("point & click"), Some(1525));
        assert_eq!(cat.id_for_name("point and click"), Some(1525));
    }

    #[test]
    fn resolve_numeric_passthrough() {
        let cat = TagCatalog::seed();
        assert_eq!(cat.resolve_query_token("392"), Some(392));
        let ids = cat
            .resolve_query_list(&["female protagonist".into(), "783".into()])
            .unwrap();
        assert_eq!(ids, vec!["392".to_string(), "783".to_string()]);
    }
}
