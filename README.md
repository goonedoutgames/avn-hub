# AVN Hub

Self-hosted **library organizer** for [F95Zone](https://f95zone.to) adult visual novels.

Browse/search F95Zone, add games by metadata, track play status and notes, customize covers, check for updates, and back up small saves/patches. No game archive storage.

Built as a lightweight Rust server with a React web client. Designed for Docker on a VPS, with API and UI on **separate ports** for easy reverse proxying.

## Architecture

| Piece | Role |
|-------|------|
| `crates/core` | Domain logic, SQLite, F95Zone client |
| `crates/auth` | Single-user password + session tokens |
| `crates/api` | REST API (`/api/v1`) |
| `crates/server` | Dual listeners: API + static web |
| `web/` | React SPA |

## Quick start (Docker Compose)

Create a `docker-compose.yml` (or use the one in this repo):

```yaml
services:
  avn-hub:
    image: ghcr.io/goonedoutgames/avn-hub:latest
    ports:
      - "8080:8080" # API
      - "8081:8081" # Web UI
    volumes:
      # SQLite DB + media cache + saves/patches
      - ./data:/data
    environment:
      AVN_HUB_API_HOST: "0.0.0.0"
      AVN_HUB_API_PORT: "8080"
      AVN_HUB_WEB_HOST: "0.0.0.0"
      AVN_HUB_WEB_PORT: "8081"
      AVN_HUB_DATA_DIR: /data
      AVN_HUB_STATIC_DIR: /app/static
      AVN_HUB_PUBLIC_API_URL: "http://127.0.0.1:8080"
      AVN_HUB_CORS_ORIGINS: "*"
      # Optional first-boot app password
      # AVN_HUB_BOOTSTRAP_PASSWORD: "changeme"
    restart: unless-stopped
```

Then:

```bash
mkdir -p data
docker compose up -d
```

On first start the container entrypoint (running as root briefly) **chowns `./data` to UID/GID 10001** so SQLite, media, saves, and patches are writable, then drops to the `avnhub` user. If you already created `./data` as root and saw `attempt to write a readonly database`, rebuild/restart — the entrypoint fixes ownership automatically.

Optional UID/GID override (match a host user):

```yaml
environment:
  AVN_HUB_UID: "1000"
  AVN_HUB_GID: "1000"
```

- Web UI: http://localhost:8081
- API: http://localhost:8080

Everything durable lives under `./data` on the host:

| Path | Contents |
|------|----------|
| `data/avn-hub.db` | Library database |
| `data/media/` | Cached covers & screenshots |
| `data/games/{id}/saves/` | Uploaded saves |
| `data/games/{id}/patches/` | Uploaded patches |

Build from source instead of pulling:

```bash
docker compose up -d --build
```

Then open **Settings** and add F95Zone credentials.

### Image

```
ghcr.io/goonedoutgames/avn-hub:latest
```

Published by GitHub Actions on pushes to `main`, `rewrite/**`, and `v*` tags. PRs to `main` build and smoke-test the image without publishing.

### Windows sidecar (Afterglow Local)

Separate from Docker: tagging `v*` also runs [`.github/workflows/windows-release.yml`](.github/workflows/windows-release.yml), which builds a native `avn-hub.exe` / `avn-hub-windows-x64.exe` and attaches them to the GitHub Release. Manual runs via **Actions → Windows sidecar release → Run workflow**.

## Environment

| Variable | Default | Description |
|----------|---------|-------------|
| `AVN_HUB_API_HOST` / `AVN_HUB_API_PORT` | `0.0.0.0` / `8080` | API **bind** address inside the container (host must be an IP/hostname like `0.0.0.0`, **not** an `https://` URL) |
| `AVN_HUB_WEB_HOST` / `AVN_HUB_WEB_PORT` | `0.0.0.0` / `8081` | Web **bind** address inside the container (same rule) |
| `AVN_HUB_DATA_DIR` | `/data` | SQLite + media + saves/patches |
| `AVN_HUB_STATIC_DIR` | `/app/static` | Built SPA assets |
| `AVN_HUB_PUBLIC_API_URL` | `http://127.0.0.1:8080` | Browser-facing API origin written into `config.json` (use your HTTPS API hostname behind a reverse proxy) |
| `AVN_HUB_CORS_ORIGINS` | `*` | Comma-separated allowed **web UI** origins (e.g. `https://avns.example.com`) |
| `AVN_HUB_BOOTSTRAP_PASSWORD` | _(unset)_ | Sets app password on first boot if none exists |
| `AVN_HUB_UID` / `AVN_HUB_GID` | `10001` / `10001` | Ownership applied to `/data` by the entrypoint (then process drops to `avnhub`) |

## Reverse proxy

Example configs live under [`deploy/nginx/`](deploy/nginx/):

| File | Layout |
|------|--------|
| [`avn-hub.conf`](deploy/nginx/avn-hub.conf) | **Recommended** — separate UI + API hostnames |
| [`avn-hub.path-based.conf`](deploy/nginx/avn-hub.path-based.conf) | Single hostname (`/` → web, `/api/` → API) |
| [`compose.swag.example.yml`](deploy/docker/compose.swag.example.yml) | SWAG / external Docker network example |

Both nginx examples include HTTPS redirects, TLS defaults, `client_max_body_size 64m` for save/patch uploads, long timeouts for F95 metadata work, and `proxy_request_buffering off` for multipart uploads.

### SWAG (linuxserver)

Keep the app listening on **8080/8081 inside the container**. Point SWAG at those container ports (`http://avn-hub:8081` UI, `http://avn-hub:8080` API). See [`deploy/docker/compose.swag.example.yml`](deploy/docker/compose.swag.example.yml).

```yaml
environment:
  AVN_HUB_API_HOST: "0.0.0.0"
  AVN_HUB_API_PORT: "8080"
  AVN_HUB_WEB_HOST: "0.0.0.0"
  AVN_HUB_WEB_PORT: "8081"
  AVN_HUB_PUBLIC_API_URL: "https://avns-api.goonedoutgames.com"
  AVN_HUB_CORS_ORIGINS: "https://avns.goonedoutgames.com"
networks:
  swag-proxy:
    external: true
```

`AVN_HUB_*_HOST` is only the bind address. Public hostnames belong in `AVN_HUB_PUBLIC_API_URL` and `AVN_HUB_CORS_ORIGINS`.

### Separate hostnames (recommended)

1. Copy and edit server names / certificate paths in `deploy/nginx/avn-hub.conf`
2. Enable the site and reload nginx
3. Set compose env to match:

```yaml
environment:
  AVN_HUB_PUBLIC_API_URL: "https://avn-api.example.com"
  AVN_HUB_CORS_ORIGINS: "https://avn.example.com"
```

### Path-based (one hostname)

Use `deploy/nginx/avn-hub.path-based.conf` and:

```yaml
environment:
  AVN_HUB_PUBLIC_API_URL: "https://avn.example.com"
  AVN_HUB_CORS_ORIGINS: "https://avn.example.com"
```

Do not expose container ports `8080`/`8081` publicly when nginx terminates TLS on the host; bind them to localhost or a Docker network only.

## Local development

### Backend

```bash
cargo run -p avn-hub-server
```

### Frontend

```bash
cd web
pnpm install
pnpm dev
```

Vite proxies `/api` to `http://127.0.0.1:8080`.

## API overview

Auth uses `Authorization: Bearer <token>` from `POST /api/v1/auth/login`.

- Catalog: `/api/v1/catalog/search`, `/browse`, `/preview`
- Library: `/api/v1/library`, `/library/add`, `/library/check-updates`
- Games: `/api/v1/games/{id}` (+ refresh, check-version, cover, saves, patches)
- Media: `/api/v1/media/{path}` (token via header or `?token=`)

## License

MIT
