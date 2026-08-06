# syntax=docker/dockerfile:1.7

# ---------------------------------------------------------------------------
# Frontend (React / Vite)
# ---------------------------------------------------------------------------
FROM node:22-bookworm AS frontend
WORKDIR /app/web

RUN corepack enable

COPY web/package.json web/pnpm-lock.yaml ./
COPY web/pnpm-workspace.yaml ./

# esbuild postinstall is allowed via onlyBuiltDependencies in pnpm-workspace.yaml
RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
    corepack prepare pnpm@10.12.4 --activate \
    && pnpm install --frozen-lockfile

COPY web/ ./
RUN pnpm build \
    && test -f dist/index.html

# ---------------------------------------------------------------------------
# Backend (Rust workspace → avn-hub binary)
# ---------------------------------------------------------------------------
FROM rust:1-bookworm AS backend
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Dependency layer: copy manifests first for better cache hits
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/auth/Cargo.toml crates/auth/Cargo.toml
COPY crates/api/Cargo.toml crates/api/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml

# Dummy sources so cargo can resolve the workspace graph and fetch deps
RUN mkdir -p crates/core/src crates/auth/src crates/api/src crates/server/src \
    && printf 'pub fn _stub() {}\n' > crates/core/src/lib.rs \
    && printf 'pub fn _stub() {}\n' > crates/auth/src/lib.rs \
    && printf 'pub fn _stub() {}\n' > crates/api/src/lib.rs \
    && printf 'fn main() {}\n' > crates/server/src/main.rs \
    && cargo fetch --locked \
    && rm -rf crates/*/src

COPY crates ./crates

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=avn-hub-target,target=/app/target \
    cargo build --locked --release -p avn-hub-server \
    && strip target/release/avn-hub \
    && cp target/release/avn-hub /tmp/avn-hub

# ---------------------------------------------------------------------------
# Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 --shell /usr/sbin/nologin avnhub

WORKDIR /app

COPY --from=backend /tmp/avn-hub /usr/local/bin/avn-hub
COPY --from=frontend /app/web/dist ./static
COPY deploy/docker/entrypoint.sh /usr/local/bin/entrypoint.sh

RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/avn-hub \
    && mkdir -p /data \
    && chown -R avnhub:avnhub /data /app/static

LABEL org.opencontainers.image.source="https://github.com/goonedoutgames/avn-hub"
LABEL org.opencontainers.image.description="AVN Hub library organizer (API + web)"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.title="AVN Hub"

ENV AVN_HUB_API_HOST=0.0.0.0 \
    AVN_HUB_API_PORT=8080 \
    AVN_HUB_WEB_HOST=0.0.0.0 \
    AVN_HUB_WEB_PORT=8081 \
    AVN_HUB_DATA_DIR=/data \
    AVN_HUB_STATIC_DIR=/app/static \
    AVN_HUB_PUBLIC_API_URL=http://127.0.0.1:8080 \
    AVN_HUB_CORS_ORIGINS=* \
    AVN_HUB_UID=10001 \
    AVN_HUB_GID=10001

EXPOSE 8080 8081
VOLUME ["/data"]

# Start as root so the entrypoint can chown bind-mounted ./data, then drop to avnhub.
USER root
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD []

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/api/v1/health || exit 1
