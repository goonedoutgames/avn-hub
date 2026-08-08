CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    source_title TEXT,
    title_custom INTEGER NOT NULL DEFAULT 0,
    f95_thread_id INTEGER UNIQUE,
    f95_url TEXT,
    version TEXT,
    developer TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    platforms TEXT NOT NULL DEFAULT '[]',
    description TEXT,
    cover_image_path TEXT,
    rating REAL,
    status TEXT,
    play_status TEXT,
    user_rating REAL,
    user_notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    local_path TEXT,
    media_type TEXT NOT NULL DEFAULT 'image',
    created_at TEXT NOT NULL
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

CREATE TABLE IF NOT EXISTS game_saves (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    uploaded_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS game_patches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    description TEXT,
    uploaded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_games_title ON games(title);
CREATE INDEX IF NOT EXISTS idx_games_thread ON games(f95_thread_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_media_game ON media(game_id);
CREATE INDEX IF NOT EXISTS idx_game_saves_game ON game_saves(game_id);
CREATE INDEX IF NOT EXISTS idx_game_patches_game ON game_patches(game_id);
