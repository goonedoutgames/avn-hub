# AVN Hub

A visual novel library and metadata manager built with **Tauri** (Rust) and **React**. Scan local game archives, match them to [F95Zone](https://f95zone.to/sam/latest_alpha/) metadata, cache cover art, and browse or download your collection from a web UI.

Runs as a **desktop app** (Tauri) or as a **Docker container** on Unraid / any Linux server.

## Features

- Scan archive folders for `.zip`, `.rar`, `.7z`, and `.bz2` files
- Search F95Zone's `latest_data.php` API for metadata matching
- Cache cover images locally in SQLite-backed storage
- Browse your matched library with tags, ratings, and versions
- Download archives to your local machine from the web UI
- Dual runtime: native Tauri desktop or headless HTTP server

## Quick Start (Desktop)

### Prerequisites

- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/) + pnpm
- Linux desktop dependencies for Tauri ([see docs](https://tauri.app/start/prerequisites/))

### Run

```bash
pnpm install
pnpm tauri:dev
```

On first launch, open **Settings** and configure:

1. **Archive folder** — path to your game archives
2. **F95Zone cookies** — required for API access (see below)

Then go to **Match** → **Scan Archives** → select a file → search/match to F95Zone.

## Quick Start (Docker / Unraid)

```bash
docker compose up -d --build
```

Open `http://your-server:8080`.

Edit `docker-compose.yml` to mount your archive folder:

```yaml
volumes:
  - ./data:/data
  - /mnt/user/game-archives:/archives:ro
```

Set the archive path in Settings to `/archives`.

### Unraid Template Notes

- **Container Port:** `8080`
- **Volume:** `/data` → app database + cached media
- **Volume:** `/archives` → read-only path to your game files
- **WebUI:** `http://[IP]:8080`

## F95Zone Authentication

F95Zone requires a logged-in session for API access. In **Settings**:

1. Enter your **username and password**, then click **Login to F95Zone**
2. The app authenticates via `f95zone.to/login/login`, caches cookies locally, and auto-refreshes when they expire
3. If login fails (2FA/CAPTCHA), paste browser cookies as a fallback

Credentials are stored locally in your SQLite database for auto re-login.

```
https://f95zone.to/sam/latest_alpha/latest_data.php?cmd=list&cat=games&search=...
```

## Project Structure

```
src/                  React frontend (Vite + Tailwind + ShadCN-style components)
src-tauri/src/
  api/                Axum HTTP server (Docker mode)
  commands.rs         Tauri IPC commands (desktop mode)
  db/                 SQLite schema and queries
  scanner.rs          Archive folder scanner
  sources/f95zone.rs  F95Zone metadata client + media cache
```

## Development

### Frontend only (proxied to server)

```bash
# Terminal 1: run the Rust server
cd src-tauri && cargo run --bin avn-hub-server

# Terminal 2: Vite dev server (proxies /api → :8080)
pnpm dev
```

### Environment Variables (Server Mode)

| Variable | Default | Description |
|----------|---------|-------------|
| `AVN_HUB_HOST` | `0.0.0.0` | Bind address |
| `AVN_HUB_PORT` | `8080` | HTTP port |
| `AVN_HUB_DATA_DIR` | `~/.local/share/avn-hub` | Database + cache |
| `AVN_HUB_STATIC_DIR` | — | Built frontend (set in Docker) |

## Workflow

1. **Settings** — set archive path and F95 cookies
2. **Match → Scan** — discover archive files in the folder
3. **Match** — select an unmatched file, review suggestions or search F95Zone, click **Match**
4. **Library** — browse matched games with cached covers, download archives

## License

MIT
