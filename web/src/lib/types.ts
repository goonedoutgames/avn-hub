export type Game = {
  id: number;
  title: string;
  source_title?: string | null;
  title_custom?: boolean;
  f95_thread_id: number | null;
  f95_url: string | null;
  version: string | null;
  developer: string | null;
  tags: string[];
  platforms?: string[];
  description: string | null;
  cover_image_path: string | null;
  rating: number | null;
  status: string | null;
  play_status: string | null;
  user_rating: number | null;
  user_notes: string | null;
  playtime_seconds?: number;
  created_at: string;
  updated_at: string;
};

export type GameSummary = {
  game: Game;
  cover_url: string | null;
  preview_urls?: string[];
};

export type ScreenshotItem = {
  full_url: string;
  cached_url: string | null;
};

export type GameSave = {
  id: number;
  game_id: number;
  path: string;
  filename: string;
  size: number;
  uploaded_at: string;
};

export type GamePatch = {
  id: number;
  game_id: number;
  path: string;
  filename: string;
  size: number;
  description: string | null;
  uploaded_at: string;
};

export type GameDetail = {
  game: Game;
  cover_url: string | null;
  cover_full_url: string | null;
  screenshots: ScreenshotItem[];
  is_custom_cover: boolean;
  saves: GameSave[];
  patches: GamePatch[];
};

export type F95SearchResult = {
  thread_id: number;
  title: string;
  creator: string;
  version: string;
  cover: string;
  screenshots: string[];
  tags: string[];
  prefixes: string[];
  platforms?: string[];
  rating: number;
  likes: number | null;
  views: number | null;
  url: string;
  date: string;
  in_library?: boolean;
  library_game_id?: number | null;
};

export type CatalogPage = {
  items: F95SearchResult[];
  page: number;
  total_pages: number;
  rows: number;
  has_more: boolean;
};

/** Browse preview — F95 thread scrape + SAM enrich without adding to the library. */
export type CatalogPreview = F95SearchResult & {
  description?: string | null;
  in_library?: boolean;
  library_game_id?: number | null;
};

export type SettingsView = {
  data_dir: string;
  f95_username: string | null;
  f95_password_set: boolean;
  f95_cookies_set: boolean;
  f95_authenticated: boolean;
  app_password_set: boolean;
  max_attachment_bytes: number;
  /** `library` = filter library by tag; `browse` = open F95 browse with tag */
  tag_click_action: "library" | "browse" | string;
  save_sync_enabled?: boolean;
  save_sync_max_per_game?: number;
  save_sync_rolling?: boolean;
  save_sync_name_format?: string;
};

export type VersionCheckResult = {
  game_id: number;
  stored_version: string | null;
  latest_version: string;
  update_available: boolean;
  f95_url: string | null;
};

export type LibraryTag = {
  tag: string;
  count: number;
};

export type LibraryPlatform = {
  platform: string;
  count: number;
};

export type CatalogTag = {
  id: number;
  name: string;
};

export type StorageStats = {
  media_cache_bytes: number;
  saves_bytes: number;
  patches_bytes: number;
  database_bytes: number;
  data_dir_bytes: number;
  data_dir: string;
};

export type AuthMe = {
  configured: boolean;
  authenticated: boolean;
};
