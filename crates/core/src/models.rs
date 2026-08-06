use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub title: String,
    /// Last known F95 / catalog title (kept even when the user customizes display title).
    #[serde(default)]
    pub source_title: Option<String>,
    /// When true, metadata refresh will not overwrite `title`.
    #[serde(default)]
    pub title_custom: bool,
    pub f95_thread_id: Option<i64>,
    pub f95_url: Option<String>,
    pub version: Option<String>,
    pub developer: Option<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub cover_image_path: Option<String>,
    pub rating: Option<f64>,
    pub status: Option<String>,
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub user_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSave {
    pub id: i64,
    pub game_id: i64,
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub uploaded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePatch {
    pub id: i64,
    pub game_id: i64,
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub description: Option<String>,
    pub uploaded_at: String,
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
    #[serde(default)]
    pub prefixes: Vec<String>,
    pub rating: f64,
    #[serde(default)]
    pub likes: Option<i64>,
    #[serde(default)]
    pub views: Option<i64>,
    pub url: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryTag {
    pub tag: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMediaRecord {
    pub id: i64,
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
    pub is_custom_cover: bool,
    pub saves: Vec<GameSave>,
    pub patches: Vec<GamePatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    pub game: Game,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheckResult {
    pub game_id: i64,
    pub stored_version: Option<String>,
    pub latest_version: String,
    pub update_available: bool,
    pub f95_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsView {
    pub data_dir: String,
    pub f95_username: Option<String>,
    pub f95_password_set: bool,
    pub f95_cookies_set: bool,
    pub f95_authenticated: bool,
    pub app_password_set: bool,
    pub max_attachment_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub media_cache_bytes: u64,
    pub saves_bytes: u64,
    pub patches_bytes: u64,
    pub database_bytes: u64,
    pub data_dir_bytes: u64,
    pub data_dir: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryFilter {
    pub search: Option<String>,
    pub play_status: Option<String>,
    pub tags: Vec<String>,
    pub tag_mode: TagMode,
    pub sort: LibrarySort,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TagMode {
    #[default]
    And,
    Or,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySort {
    #[default]
    TitleAsc,
    TitleDesc,
    UpdatedDesc,
    RatingDesc,
    UserRatingDesc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGameUserData {
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub user_notes: Option<String>,
    /// Custom display title for the library.
    pub title: Option<String>,
    /// When true, restore `title` from `source_title` and clear the custom flag.
    pub reset_title: Option<bool>,
    pub description: Option<String>,
}
