/** Prefer locally cached full-resolution media over remote hotlink URLs. */
export function bestImageUrl(
  cached?: string | null,
  remote?: string | null,
): string | null {
  return cached ?? remote ?? null;
}

/** Pick the best URL from a screenshot/cover item. */
export function screenshotDisplayUrl(shot: {
  cached_url?: string | null;
  full_url: string;
}): string {
  return shot.cached_url ?? shot.full_url;
}
