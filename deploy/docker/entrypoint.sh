#!/bin/sh
# Ensure /data (and SQLite WAL companions) are writable by the app user, then drop root.
set -eu

DATA_DIR="${AVN_HUB_DATA_DIR:-/data}"
APP_UID="${AVN_HUB_UID:-10001}"
APP_GID="${AVN_HUB_GID:-10001}"

mkdir -p "$DATA_DIR/media" "$DATA_DIR/games"

fix_data_ownership() {
  # Host bind mounts often arrive as root (or the compose user). SQLite needs
  # write access to the directory for -wal/-shm and to the .db itself.
  chown -R "${APP_UID}:${APP_GID}" "$DATA_DIR" || true
  chmod -R u+rwX,g+rX,o+rX "$DATA_DIR" || true
  chmod u+rwx "$DATA_DIR" || true
}

if [ "$(id -u)" = "0" ]; then
  fix_data_ownership
  exec setpriv --reuid="$APP_UID" --regid="$APP_GID" --clear-groups -- /usr/local/bin/avn-hub "$@"
fi

# Already non-root (e.g. compose `user:`). Fail fast if the volume is not writable.
if ! touch "$DATA_DIR/.write-test" 2>/dev/null; then
  echo "avn-hub: cannot write to $DATA_DIR — chown it to $(id -u):$(id -g), or run the image as root so the entrypoint can fix ownership" >&2
  exit 1
fi
rm -f "$DATA_DIR/.write-test"

exec /usr/local/bin/avn-hub "$@"
