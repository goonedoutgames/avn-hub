CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    archive_path TEXT NOT NULL UNIQUE,
    archive_filename TEXT NOT NULL,
    archive_size INTEGER NOT NULL DEFAULT 0,
    f95_thread_id INTEGER,
    f95_url TEXT,
    version TEXT,
    developer TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    description TEXT,
    cover_image_path TEXT,
    rating REAL,
    status TEXT,
    matched INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS metadata_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT,
    data TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    UNIQUE(source, external_id)
);

CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    local_path TEXT,
    media_type TEXT NOT NULL DEFAULT 'image',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_games_matched ON games(matched);
CREATE INDEX IF NOT EXISTS idx_games_title ON games(title);
CREATE INDEX IF NOT EXISTS idx_metadata_cache_source ON metadata_cache(source, external_id);
