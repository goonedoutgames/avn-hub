export type PlayStatus = "unplayed" | "playing" | "completed" | "dropped";

export type Platform =
  | "windows"
  | "linux"
  | "windows_linux"
  | "mac"
  | "android"
  | "unknown";

export type UploadKind = "archive" | "save" | "patch";

export const PLATFORMS: Platform[] = [
  "windows",
  "linux",
  "windows_linux",
  "mac",
  "android",
  "unknown",
];

export function platformLabel(platform: string): string {
  switch (platform) {
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    case "windows_linux":
      return "Windows & Linux";
    case "mac":
      return "macOS";
    case "android":
      return "Android";
    default:
      return "Unknown";
  }
}

/** Best-effort platform guess from filename (mirrors backend heuristics). */
export function guessPlatformFromFilename(filename: string): Platform {
  const lower = filename.toLowerCase();
  if (
    lower.includes("android") ||
    lower.endsWith(".apk") ||
    lower.includes("_apk") ||
    lower.includes("-apk")
  ) {
    return "android";
  }
  const hasPc =
    lower.split(/[^a-z0-9]+/).includes("pc") ||
    lower.includes("_pc") ||
    lower.includes("-pc") ||
    lower.endsWith("_pc") ||
    lower.endsWith("-pc");
  const hasWin =
    lower.includes("win64") ||
    lower.includes("win32") ||
    lower.includes("windows") ||
    lower.includes("_win") ||
    lower.includes("-win") ||
    lower.includes("winlin") ||
    lower.includes("win_lin") ||
    lower.includes("win-lin");
  const hasLin =
    lower.includes("linux") ||
    lower.includes("_lin") ||
    lower.includes("-lin") ||
    lower.includes("winlin") ||
    lower.includes("win_lin") ||
    lower.includes("win-lin");
  if (hasPc || (hasWin && hasLin)) return "windows_linux";
  if (lower.includes("macos") || lower.includes("_mac") || lower.endsWith(".dmg")) {
    return "mac";
  }
  if (hasLin) return "linux";
  if (hasWin) return "windows";
  return "unknown";
}

export interface Game {
  id: number;
  title: string;
  archive_path: string;
  archive_filename: string;
  archive_size: number;
  f95_thread_id: number | null;
  f95_url: string | null;
  version: string | null;
  developer: string | null;
  tags: string[];
  description: string | null;
  cover_image_path: string | null;
  rating: number | null;
  status: string | null;
  play_status: PlayStatus | null;
  user_rating: number | null;
  user_notes: string | null;
  matched: boolean;
  created_at: string;
  updated_at: string;
}

export interface GameResponse {
  game: Game;
  cover_url: string | null;
  cover_full_url?: string | null;
  preview_urls?: string[];
  platform_archives?: GamePlatformArchive[];
}

export interface ArchiveEntry {
  id: number;
  path: string;
  filename: string;
  size: number;
  platform: Platform;
  matched: boolean;
  game_id: number | null;
}

export interface GamePlatformArchive {
  id: number;
  game_id: number;
  platform: Platform;
  path: string;
  filename: string;
  size: number;
  is_default: boolean;
  uploaded_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface GameSave {
  id: number;
  game_id: number;
  path: string;
  filename: string;
  size: number;
  uploaded_at: string;
}

export interface GamePatch {
  id: number;
  game_id: number;
  path: string;
  filename: string;
  size: number;
  description: string | null;
  uploaded_at: string;
}

export interface GameAttachments {
  platform_archives: GamePlatformArchive[];
  saves: GameSave[];
  patches: GamePatch[];
}

export interface F95SearchResult {
  thread_id: number;
  title: string;
  creator: string;
  version: string;
  cover: string;
  screenshots: string[];
  tags: string[];
  rating: number;
  url: string;
  date: string;
}

export interface LibraryTag {
  tag: string;
  count: number;
}

export type TagFilterMode = "and" | "or";

export type LibrarySort =
  | "title"
  | "title_desc"
  | "f95_rating"
  | "f95_rating_asc"
  | "user_rating"
  | "user_rating_asc"
  | "play_status"
  | "play_status_desc";

export interface LibraryListParams {
  search?: string;
  tags?: string;
  tagsMode?: TagFilterMode;
  playStatus?: PlayStatus[];
  minF95Rating?: number;
  maxF95Rating?: number;
  minUserRating?: number;
  maxUserRating?: number;
  sort?: LibrarySort;
}

export interface Settings {
  archive_path: string;
  data_dir: string;
  f95_username: string | null;
  f95_password_set: boolean;
  f95_cookies: string | null;
  f95_authenticated: boolean;
  http_auth_configured: boolean;
  http_auth_username: string | null;
}

export interface AuthStatus {
  configured: boolean;
  authenticated: boolean;
  username: string | null;
}

export interface HttpLoginRequest {
  username: string;
  password: string;
}

export interface ScanResult {
  added: number;
  updated: number;
  total: number;
}

export interface ScreenshotItem {
  full_url: string;
  cached_url: string | null;
}

export interface GameDetail {
  game: Game;
  cover_url: string | null;
  cover_full_url?: string | null;
  screenshots: ScreenshotItem[];
  is_custom_cover: boolean;
  attachments: GameAttachments;
}

export interface UpdateGameUserDataRequest {
  play_status: PlayStatus | null;
  user_rating: number | null;
  user_notes: string | null;
}

export interface StorageStats {
  archives_bytes: number;
  media_cache_bytes: number;
  database_bytes: number;
  data_dir_bytes: number;
  archive_path: string;
  data_dir: string;
  archive_volume_total: number | null;
  archive_volume_available: number | null;
  data_volume_total: number | null;
  data_volume_available: number | null;
}

export interface MatchRequest {
  archive_id?: number;
  archive_path?: string;
  thread_id: number;
  hint?: F95SearchResult;
  platform?: Platform;
}

export interface SetArchivePlatformRequest {
  platform: Platform;
  reorganize?: boolean;
}

export interface MigrationArchiveItem {
  id: number;
  game_id: number;
  game_title: string;
  filename: string;
  platform: Platform;
  path: string;
  is_default: boolean;
  is_legacy_path: boolean;
  needs_platform: boolean;
}

export interface MigrationStatus {
  total_archives: number;
  needs_attention: number;
  legacy_paths: number;
  unknown_platforms: number;
  archives: MigrationArchiveItem[];
}

export interface ReorganizeResult {
  moved: number;
  skipped_unknown: number;
  skipped_already_structured: number;
  skipped_missing: number;
  failed: number;
  errors: string[];
}

export interface VersionCheckResult {
  stored_version: string | null;
  latest_version: string;
  update_available: boolean;
  f95_url: string | null;
}

export interface UpdateSettingsRequest {
  archive_path?: string;
  f95_username?: string;
  f95_password?: string;
  f95_cookies?: string;
  http_auth_username?: string;
  http_auth_password?: string;
  http_auth_remove?: boolean;
}

export interface F95LoginRequest {
  username?: string;
  password?: string;
}

export interface F95LoginResult {
  success: boolean;
  message: string;
}
