# AVN Hub — agent notes

Library and catalog UX must behave like **Steam**, not like an unbounded admin dump.

## Steam-like rules

- **Remember the session.** Sort, play status, rating filters, and other library view choices belong in the URL or `localStorage` so they survive reload. Do not reset to “Title A–Z / any status” on every visit.
- **Keep filters compact.** Tag clouds stay collapsed (about 5 chips + a `+N more` badge) until the user expands them. Selected tags stay visible. Do not render 60 genre chips as the default chrome.
- **Display is not a filter.** Grid/list, card size, and other view options are separate from search/sort/tag filters. Never bury display controls under a long tag list.
- **Sort parity with Afterglow.** `GET /api/v1/library?sort=` must support at least:
  - `title_asc`, `title_desc`
  - `updated_desc`
  - `rating_desc` (F95)
  - `user_rating_desc`
  - `playtime_desc`
  Afterglow must not need a client-only sort the hub cannot do.
- **Media is cached.** Hub already stores covers under the data dir. Clients (web + Afterglow) should read cached `/api/v1/media/...` URLs, not re-decode F95 originals on every paint. Cover changes (custom cover, metadata refresh) bump `updated_at` / `cover_image_path` so clients can revalidate instead of downloading blindly.

## API contract

Canonical surface: [`openapi/openapi.yaml`](openapi/openapi.yaml). Change that file in the same PR as `crates/api/src/routes.rs`.
