import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { PlayStatusBadge, StarRating } from "@/components/StarRating";
import { useToast } from "@/context/ToastContext";
import { api, mediaUrl } from "@/lib/api";
import type { GameSummary, LibraryTag, VersionCheckResult } from "@/lib/types";

export function LibraryPage() {
  const toast = useToast();
  const [games, setGames] = useState<GameSummary[]>([]);
  const [tags, setTags] = useState<LibraryTag[]>([]);
  const [search, setSearch] = useState("");
  const [playStatus, setPlayStatus] = useState("");
  const [userRatingFilter, setUserRatingFilter] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [sort, setSort] = useState("title_asc");
  const [error, setError] = useState<string | null>(null);
  const [updates, setUpdates] = useState<VersionCheckResult[] | null>(null);
  const [busy, setBusy] = useState(false);

  const load = async () => {
    setError(null);
    try {
      const [list, tagList] = await Promise.all([
        api.library({
          search: search || undefined,
          play_status: playStatus || undefined,
          user_rating: userRatingFilter || undefined,
          tags: selectedTags.join(",") || undefined,
          sort,
        }),
        api.libraryTags(),
      ]);
      setGames(list);
      setTags(tagList);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to load library";
      setError(msg);
      toast.error(msg);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playStatus, userRatingFilter, sort, selectedTags.join("|")]);

  const toggleTag = (tag: string) => {
    setSelectedTags((prev) =>
      prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag],
    );
  };

  const checkUpdates = async () => {
    setBusy(true);
    setError(null);
    try {
      const results = await api.checkAllUpdates();
      const available = results.filter((r) => r.update_available);
      setUpdates(available);
      toast.success(
        available.length === 0
          ? "All games are up to date"
          : `${available.length} update${available.length === 1 ? "" : "s"} available`,
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Update check failed";
      setError(msg);
      toast.error(msg);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Library</h1>
          <p className="page-subtitle">{games.length} games</p>
        </div>
        <div className="toolbar">
          <button type="button" className="btn" disabled={busy} onClick={() => void checkUpdates()}>
            {busy ? "Checking…" : "Check updates"}
          </button>
          <Link to="/browse" className="btn btn-primary">
            Add games
          </Link>
        </div>
      </div>

      <div className="card card-section grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
        <label className="block text-sm sm:col-span-2 lg:col-span-2">
          <span className="field-label">Search</span>
          <div className="flex flex-col gap-2 sm:flex-row">
            <input
              className="input"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void load()}
              placeholder="Title, developer, tags…"
            />
            <button type="button" className="btn shrink-0" onClick={() => void load()}>
              Search
            </button>
          </div>
        </label>
        <label className="block text-sm">
          <span className="field-label">Status</span>
          <select
            className="input"
            value={playStatus}
            onChange={(e) => setPlayStatus(e.target.value)}
          >
            <option value="">Any status</option>
            <option value="unplayed">Unplayed</option>
            <option value="playing">Playing</option>
            <option value="completed">Completed</option>
            <option value="dropped">Dropped</option>
          </select>
        </label>
        <label className="block text-sm">
          <span className="field-label">Your rating</span>
          <select
            className="input"
            value={userRatingFilter}
            onChange={(e) => setUserRatingFilter(e.target.value)}
          >
            <option value="">Any rating</option>
            <option value="unrated">Unrated</option>
            <option value="1">1★ and up</option>
            <option value="2">2★ and up</option>
            <option value="3">3★ and up</option>
            <option value="4">4★ and up</option>
            <option value="5">5★ only</option>
          </select>
        </label>
        <label className="block text-sm">
          <span className="field-label">Sort</span>
          <select className="input" value={sort} onChange={(e) => setSort(e.target.value)}>
            <option value="title_asc">Title A–Z</option>
            <option value="title_desc">Title Z–A</option>
            <option value="updated_desc">Recently updated</option>
            <option value="rating_desc">F95 rating</option>
            <option value="user_rating_desc">Your rating</option>
          </select>
        </label>
      </div>

      {tags.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {tags.slice(0, 40).map((t) => (
            <button
              key={t.tag}
              type="button"
              className={`rounded-full border px-2.5 py-1 text-xs ${
                selectedTags.includes(t.tag)
                  ? "border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_20%,transparent)]"
                  : "border-[var(--border)] text-[var(--muted)]"
              }`}
              onClick={() => toggleTag(t.tag)}
            >
              {t.tag} ({t.count})
            </button>
          ))}
        </div>
      )}

      {error && <p className="text-sm text-[var(--danger)]">{error}</p>}

      {updates && (
        <div className="card card-section stack">
          <h2 className="m-0 text-base font-semibold">Updates available ({updates.length})</h2>
          {updates.length === 0 ? (
            <p className="muted text-sm">All games are up to date.</p>
          ) : (
            <ul className="space-y-1 text-sm">
              {updates.map((u) => (
                <li key={u.game_id}>
                  <Link to={`/game/${u.game_id}`}>
                    Game #{u.game_id}: {u.stored_version ?? "?"} → {u.latest_version}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {games.map(({ game, cover_url }) => (
          <Link
            key={game.id}
            to={`/game/${game.id}`}
            className="card group overflow-hidden transition hover:border-[var(--accent-dim)]"
          >
            <div className="relative aspect-[16/9] bg-[var(--bg-soft)]">
              {mediaUrl(cover_url) ? (
                <img
                  src={mediaUrl(cover_url)!}
                  alt=""
                  className="h-full w-full object-cover transition duration-300 group-hover:scale-[1.02]"
                  loading="lazy"
                />
              ) : (
                <div className="flex h-full items-center justify-center p-3 text-center text-sm text-[var(--muted)]">
                  {game.title}
                </div>
              )}
              <div className="absolute top-2 left-2">
                <PlayStatusBadge status={game.play_status} />
              </div>
              {game.version && (
                <span className="absolute right-2 bottom-2 rounded bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white">
                  {game.version.startsWith("v") || game.version.startsWith("V")
                    ? game.version
                    : `v${game.version}`}
                </span>
              )}
            </div>
            <div className="space-y-2 p-3">
              <div className="line-clamp-2 text-sm font-semibold leading-snug">{game.title}</div>
              <div className="muted text-xs">{game.developer ?? "Unknown"}</div>
              <div className="rating-row">
                <StarRating
                  label="F95"
                  value={game.rating != null && game.rating > 0 ? game.rating : null}
                  size="sm"
                  showValue
                />
                <StarRating
                  label="You"
                  value={game.user_rating}
                  size="sm"
                  showValue
                />
              </div>
              {game.tags.filter((t) => !/^\d+$/.test(t)).length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {game.tags
                    .filter((t) => !/^\d+$/.test(t))
                    .slice(0, 4)
                    .map((t) => (
                      <span
                        key={t}
                        className="rounded border border-[var(--border)] px-1.5 py-0.5 text-[10px] text-[var(--muted)]"
                      >
                        {t}
                      </span>
                    ))}
                </div>
              )}
            </div>
          </Link>
        ))}
      </div>

      {games.length === 0 && !error && (
        <p className="muted py-12 text-center">
          No games yet.{" "}
          <Link to="/browse" className="text-[var(--accent)]">
            Browse F95Zone
          </Link>{" "}
          to add some.
        </p>
      )}
    </div>
  );
}
