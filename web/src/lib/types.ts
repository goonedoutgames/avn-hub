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
  description: string | null;
  cover_image_path: string | null;
  rating: number | null;
  status: string | null;
  play_status: string | null;
  user_rating: number | null;
  user_notes: string | null;
  created_at: string;
  updated_at: string;
};

export type GameSummary = {
  game: Game;
  cover_url: string | null;
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
  rating: number;
  likes: number | null;
  views: number | null;
  url: string;
  date: string;
};

export type SettingsView = {
  data_dir: string;
  f95_username: string | null;
  f95_password_set: boolean;
  f95_cookies_set: boolean;
  f95_authenticated: boolean;
  app_password_set: boolean;
  max_attachment_bytes: number;
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
