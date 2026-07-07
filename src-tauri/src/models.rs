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
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub user_notes: Option<String>,
    pub matched: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub platform: String,
    pub matched: bool,
    pub game_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePlatformArchive {
    pub id: i64,
    pub game_id: i64,
    pub platform: String,
    pub path: String,
    pub filename: String,
    pub size: i64,
    pub is_default: bool,
    pub uploaded_at: Option<String>,
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
pub struct GameAttachments {
    pub platform_archives: Vec<GamePlatformArchive>,
    pub saves: Vec<GameSave>,
    pub patches: Vec<GamePatch>,
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
    pub http_auth_configured: bool,
    pub http_auth_username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub configured: bool,
    pub authenticated: bool,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpLoginResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub added: usize,
    pub updated: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRequest {
    #[serde(default)]
    pub archive_id: Option<i64>,
    #[serde(default)]
    pub archive_path: Option<String>,
    pub thread_id: i64,
    pub hint: Option<F95SearchResult>,
    #[serde(default)]
    pub platform: Option<String>,
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
pub struct UpdateGameUserDataRequest {
    pub play_status: Option<String>,
    pub user_rating: Option<f64>,
    pub user_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub archives_bytes: i64,
    pub media_cache_bytes: u64,
    pub database_bytes: u64,
    pub data_dir_bytes: u64,
    pub archive_path: String,
    pub data_dir: String,
    pub archive_volume_total: Option<u64>,
    pub archive_volume_available: Option<u64>,
    pub data_volume_total: Option<u64>,
    pub data_volume_available: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetail {
    pub game: Game,
    pub cover_url: Option<String>,
    pub cover_full_url: Option<String>,
    pub screenshots: Vec<ScreenshotItem>,
    pub is_custom_cover: bool,
    pub attachments: GameAttachments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResponse {
    pub game: Game,
    pub cover_url: Option<String>,
    pub cover_full_url: Option<String>,
    pub preview_urls: Vec<String>,
    pub platform_archives: Vec<GamePlatformArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationArchiveItem {
    pub id: i64,
    pub game_id: i64,
    pub game_title: String,
    pub filename: String,
    pub platform: String,
    pub path: String,
    pub is_default: bool,
    pub is_legacy_path: bool,
    pub needs_platform: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub total_archives: usize,
    pub needs_attention: usize,
    pub legacy_paths: usize,
    pub unknown_platforms: usize,
    pub archives: Vec<MigrationArchiveItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorganizeResult {
    pub moved: usize,
    pub skipped_unknown: usize,
    pub skipped_already_structured: usize,
    pub skipped_missing: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetArchivePlatformRequest {
    pub platform: String,
    #[serde(default = "default_true")]
    pub reorganize: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheckResult {
    pub stored_version: Option<String>,
    pub latest_version: String,
    pub update_available: bool,
    pub f95_url: Option<String>,
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
    pub http_auth_username: Option<String>,
    pub http_auth_password: Option<String>,
    pub http_auth_remove: Option<bool>,
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
