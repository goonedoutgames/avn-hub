use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub archive_path: String,
    pub archive_filename: String,
    pub archive_size: i64,
    pub f95_thread_id: Option<i64>,
    pub f95_url: Option<String>,
    pub version: Option<String>,
    pub developer: Option<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub cover_image_path: Option<String>,
    pub rating: Option<f64>,
    pub status: Option<String>,
    pub matched: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub matched: bool,
    pub game_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F95SearchResult {
    pub thread_id: i64,
    pub title: String,
    pub creator: String,
    pub version: String,
    pub cover: String,
    #[serde(default)]
    pub screenshots: Vec<String>,
    pub tags: Vec<String>,
    pub rating: f64,
    pub url: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryTag {
    pub tag: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub archive_path: String,
    pub data_dir: String,
    pub f95_username: Option<String>,
    pub f95_password_set: bool,
    pub f95_cookies: Option<String>,
    pub f95_authenticated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub added: usize,
    pub updated: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRequest {
    pub archive_path: String,
    pub thread_id: i64,
    pub hint: Option<F95SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMediaRecord {
    pub media_type: String,
    pub source_url: String,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotItem {
    pub full_url: String,
    pub cached_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetail {
    pub game: Game,
    pub cover_url: Option<String>,
    pub cover_full_url: Option<String>,
    pub screenshots: Vec<ScreenshotItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResponse {
    pub game: Game,
    pub cover_url: Option<String>,
    pub cover_full_url: Option<String>,
    pub preview_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCoverRequest {
    pub screenshot_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub archive_path: Option<String>,
    pub f95_username: Option<String>,
    pub f95_password: Option<String>,
    pub f95_cookies: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F95LoginRequest {
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F95LoginResult {
    pub success: bool,
    pub message: String,
}
