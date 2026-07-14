# syntax=docker/dockerfile:1

FROM node:22-bookworm AS frontend
WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY index.html vite.config.ts tsconfig.json tsconfig.node.json ./
COPY public ./public
COPY src ./src
RUN pnpm build

FROM rust:1-bookworm AS backend
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./src-tauri/
COPY src-tauri/build.rs ./src-tauri/
COPY src-tauri/src ./src-tauri/src
WORKDIR /app/src-tauri
RUN cargo build --release --no-default-features --bin avn-hub-server \
    && strip target/release/avn-hub-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/src-tauri/target/release/avn-hub-server /usr/local/bin/avn-hub-server
COPY --from=frontend /app/dist ./static

LABEL org.opencontainers.image.source="https://github.com/goonedoutgames/avn-hub"
LABEL org.opencontainers.image.description="AVN Hub headless server"
LABEL org.opencontainers.image.licenses="MIT"

ENV AVN_HUB_HOST=0.0.0.0
ENV AVN_HUB_PORT=8080
ENV AVN_HUB_DATA_DIR=/data
ENV AVN_HUB_STATIC_DIR=/app/static

EXPOSE 8080
VOLUME ["/data", "/archives"]

CMD ["avn-hub-server"]
