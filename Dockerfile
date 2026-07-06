# syntax=docker/dockerfile:1

FROM node:22-bookworm AS frontend
WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY index.html vite.config.ts tsconfig.json tsconfig.node.json ./
COPY src ./src
RUN pnpm build

FROM rust:1.88-bookworm AS backend
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock* ./src-tauri/
COPY src-tauri/build.rs src-tauri/tauri.conf.json ./src-tauri/
COPY src-tauri/capabilities ./src-tauri/capabilities
COPY src-tauri/icons ./src-tauri/icons
COPY src-tauri/src ./src-tauri/src
WORKDIR /app/src-tauri
RUN cargo build --release --bin avn-hub-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/src-tauri/target/release/avn-hub-server /usr/local/bin/avn-hub-server
COPY --from=frontend /app/dist ./static

ENV AVN_HUB_HOST=0.0.0.0
ENV AVN_HUB_PORT=8080
ENV AVN_HUB_DATA_DIR=/data
ENV AVN_HUB_STATIC_DIR=/app/static

EXPOSE 8080
VOLUME ["/data", "/archives"]

CMD ["avn-hub-server"]
