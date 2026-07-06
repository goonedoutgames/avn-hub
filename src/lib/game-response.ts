import type { Game, GameResponse } from "./types";

/** Normalize API responses — server historically flattened game fields. */
export function normalizeGameResponse(item: unknown): GameResponse | null {
  if (!item || typeof item !== "object") return null;
  const obj = item as Record<string, unknown>;

  if (obj.game && typeof obj.game === "object") {
    return {
      game: obj.game as Game,
      cover_url: (obj.cover_url as string | null) ?? null,
      cover_full_url: (obj.cover_full_url as string | null) ?? null,
      preview_urls: Array.isArray(obj.preview_urls)
        ? (obj.preview_urls as string[])
        : [],
    };
  }

  if ("id" in obj && "title" in obj && "archive_path" in obj) {
    const { cover_url, cover_full_url, preview_urls, ...rest } = obj;
    return {
      game: rest as unknown as Game,
      cover_url: (cover_url as string | null) ?? null,
      cover_full_url: (cover_full_url as string | null) ?? null,
      preview_urls: Array.isArray(preview_urls)
        ? (preview_urls as string[])
        : [],
    };
  }

  return null;
}

export function normalizeGameList(items: unknown): GameResponse[] {
  if (!Array.isArray(items)) return [];
  return items
    .map(normalizeGameResponse)
    .filter((g): g is GameResponse => g !== null);
}
