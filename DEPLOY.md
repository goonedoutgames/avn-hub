# Deploying AVN Hub

## Docker (recommended)

Published image: `ghcr.io/goonedoutgames/avn-hub:latest`

```bash
# From a checkout, or with your own compose file pointing at the image:
docker compose pull
docker compose up -d
```

Mount persistent volumes for `/data` (SQLite + cache) and `/archives` (game files, **read-write**). The image seeds Settings → archive path to `/archives` via `AVN_HUB_ARCHIVE_PATH` on first boot — keep that container path (not a host path).

CI builds and tags the image on `main` and `v*` tags via `.github/workflows/docker.yml`.

---

## VPS / bare metal (no Docker)

The server runs a single binary (`avn-hub-server`) plus a built frontend (`dist/`).

## Update workflow

After pulling code changes on your **build machine**:

```bash
pnpm install
pnpm build
cd src-tauri && cargo build --release --bin avn-hub-server
```

Copy to the VPS:

```bash
scp src-tauri/target/release/avn-hub-server root@YOUR_VPS:/opt/avn-hub/
scp -r dist/. root@YOUR_VPS:/opt/avn-hub/static/
ssh root@YOUR_VPS "systemctl restart avn-hub"
```

That's it — database and archives in `/opt/avn-hub/data` and `/opt/avn-hub/archives` are untouched.

## First-time layout on the server

```
/opt/avn-hub/
  avn-hub-server      # Rust binary
  static/             # pnpm build output (index.html, assets/)
  data/               # SQLite + cached media (AVN_HUB_DATA_DIR)
  archives/           # game .zip/.rar/.7z files
```

## systemd

`/etc/systemd/system/avn-hub.service`:

```ini
[Unit]
Description=AVN Hub
After=network-online.target

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/opt/avn-hub
Environment=AVN_HUB_HOST=127.0.0.1
Environment=AVN_HUB_PORT=8080
Environment=AVN_HUB_DATA_DIR=/opt/avn-hub/data
Environment=AVN_HUB_STATIC_DIR=/opt/avn-hub/static
ExecStart=/opt/avn-hub/avn-hub-server
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

## nginx (reverse proxy + TUS uploads)

TUS resumable uploads need large bodies and optional request buffering off:

```nginx
server {
    listen 443 ssl;
    server_name avns.example.com;

    client_max_body_size 0;
    proxy_request_buffering off;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 3600s;
        proxy_buffering off;
    }
}
```

## Web login

In **Settings → Web Login** set a username and password. After that:

- Visitors must sign in once; session cookie lasts **7 days**
- Until credentials are set, a banner warns you each visit (app still works)

## Uploading archives from the browser

On the **Match** page, use **Upload archive** (TUS). Files land in the configured archive folder. Run **Scan Archives** afterward if the upload didn't register automatically.

## Build on a low-RAM VPS

`cargo build` pulls Tauri/GTK dev deps and uses a lot of RAM. Prefer building on your desktop and copying the release binary, or add swap before compiling on the VPS.
