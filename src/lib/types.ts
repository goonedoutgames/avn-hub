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
  matched: boolean;
  created_at: string;
  updated_at: string;
}

export interface GameResponse {
  game: Game;
  cover_url: string | null;
  cover_full_url?: string | null;
  preview_urls?: string[];
}

export interface ArchiveEntry {
  path: string;
  filename: string;
  size: number;
  matched: boolean;
  game_id: number | null;
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

export interface Settings {
  archive_path: string;
  data_dir: string;
  f95_username: string | null;
  f95_password_set: boolean;
  f95_cookies: string | null;
  f95_authenticated: boolean;
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
}

export interface MatchRequest {
  archive_path: string;
  thread_id: number;
  hint?: F95SearchResult;
}

export interface UpdateSettingsRequest {
  archive_path?: string;
  f95_username?: string;
  f95_password?: string;
  f95_cookies?: string;
}

export interface F95LoginRequest {
  username?: string;
  password?: string;
}

export interface F95LoginResult {
  success: boolean;
  message: string;
}
