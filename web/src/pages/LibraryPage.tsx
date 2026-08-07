import { useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { ArrowUpDown, SlidersHorizontal } from "lucide-react";
import { PlayStatusBadge, StarRating } from "@/components/StarRating";
import { HoverMedia } from "@/components/HoverMedia";
import { SideDrawer } from "@/components/SideDrawer";
import { TagBadges } from "@/components/TagBadges";
import { useToast } from "@/context/ToastContext";
import { api, mediaUrl } from "@/lib/api";
import type { GameSummary, LibraryTag, VersionCheckResult } from "@/lib/types";

function parseTagsParam(raw: string | null): string[] {
  if (!raw) return [];
  return raw
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
}

export function LibraryPage() {
  const toast = useToast();
  const [searchParams, setSearchParams] = useSearchParams();
  const [games, setGames] = useState<GameSummary[]>([]);
  const [tags, setTags] = useState<LibraryTag[]>([]);
  const [search, setSearch] = useState("");
  const [playStatus, setPlayStatus] = useState("");
  const [userRatingFilter, setUserRatingFilter] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>(() =>
    parseTagsParam(searchParams.get("tags")),
  );
  const [sort, setSort] = useState("title_asc");
  const [error, setError] = useState<string | null>(null);
  const [updates, setUpdates] = useState<VersionCheckResult[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [sortOpen, setSortOpen] = useState(false);

  useEffect(() => {
    const fromUrl = parseTagsParam(searchParams.get("tags"));
    setSelectedTags((prev) => {
      const same =
        prev.length === fromUrl.length && prev.every((t, i) => t === fromUrl[i]);
      return same ? prev : fromUrl;
    });
  }, [searchParams]);

  const syncTagsToUrl = (next: string[]) => {
    setSelectedTags(next);
    const nextParams = new URLSearchParams(searchParams);
    if (next.length) nextParams.set("tags", next.join(","));
    else nextParams.delete("tags");
    setSearchParams(nextParams, { replace: true });
  };

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
    const next = selectedTags.includes(tag)
      ? selectedTags.filter((t) => t !== tag)
      : [...selectedTags, tag];
    syncTagsToUrl(next);
  };

  const clearFilters = () => {
    setSearch("");
    setPlayStatus("");
    setUserRatingFilter("");
    syncTagsToUrl([]);
  };

  const filterCount = useMemo(() => {
    let n = 0;
    if (search.trim()) n += 1;
    if (playStatus) n += 1;
    if (userRatingFilter) n += 1;
    n += selectedTags.length;
    return n;
  }, [search, playStatus, userRatingFilter, selectedTags]);

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

  const sortLabel: Record<string, string> = {
    title_asc: "Title A–Z",
    title_desc: "Title Z–A",
    updated_desc: "Recently updated",
    rating_desc: "F95 rating",
    user_rating_desc: "Your rating",
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Library</h1>
          <p className="page-subtitle">
            {games.length} games
            {filterCount > 0 ? ` · ${filterCount} filter${filterCount === 1 ? "" : "s"}` : ""}
            {` · ${sortLabel[sort] ?? sort}`}
          </p>
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
        {games.map(({ game, cover_url, preview_urls }) => {
          const gallery = (preview_urls?.length ? preview_urls : cover_url ? [cover_url] : [])
            .map((u) => mediaUrl(u))
            .filter((u): u is string => Boolean(u));

          return (
          <Link
            key={game.id}
            to={`/game/${game.id}`}
            className="card group overflow-hidden transition hover:border-[var(--accent-dim)]"
          >
            {gallery.length > 0 ? (
              <HoverMedia
                images={gallery}
                className="aspect-[16/9]"
                imgClassName="transition duration-500 group-hover:scale-[1.02]"
              >
                <div className="absolute top-2 left-2 z-[1]">
                  <PlayStatusBadge status={game.play_status} />
                </div>
                {game.version && (
                  <span className="absolute right-2 bottom-2 z-[1] rounded bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white">
                    {game.version.startsWith("v") || game.version.startsWith("V")
                      ? game.version
                      : `v${game.version}`}
                  </span>
                )}
              </HoverMedia>
            ) : (
              <div className="relative aspect-[16/9] bg-[var(--bg-soft)]">
                <div className="flex h-full items-center justify-center p-3 text-center text-sm text-[var(--muted)]">
                  {game.title}
                </div>
                <div className="absolute top-2 left-2">
                  <PlayStatusBadge status={game.play_status} />
                </div>
              </div>
            )}
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
                <StarRating label="You" value={game.user_rating} size="sm" showValue />
              </div>
              <TagBadges tags={game.tags} limit={4} />
            </div>
          </Link>
          );
        })}
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

      <div className="fab-cluster">
        <button type="button" className="fab" onClick={() => setFilterOpen(true)}>
          <SlidersHorizontal className="h-4 w-4" />
          Filters
          {filterCount > 0 && <span className="fab-badge">{filterCount}</span>}
        </button>
        <button type="button" className="fab" onClick={() => setSortOpen(true)}>
          <ArrowUpDown className="h-4 w-4" />
          Sort
        </button>
      </div>

      <SideDrawer
        open={filterOpen}
        title="Library filters"
        subtitle="Search, status, rating, and tags"
        onClose={() => setFilterOpen(false)}
        footer={
          <>
            <button type="button" className="btn" onClick={clearFilters}>
              Clear
            </button>
            <button
              type="button"
              className="btn btn-primary ml-auto"
              onClick={() => {
                void load();
                setFilterOpen(false);
              }}
            >
              Apply search
            </button>
          </>
        }
      >
        <label className="block text-sm">
          <span className="field-label">Search</span>
          <input
            className="input"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void load();
                setFilterOpen(false);
              }
            }}
            placeholder="Title, developer, tags…"
          />
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
        {tags.length > 0 && (
          <section className="stack">
            <div className="field-label">Tags in your library</div>
            <div className="flex flex-wrap gap-1.5">
              {tags.slice(0, 60).map((t) => (
                <button
                  key={t.tag}
                  type="button"
                  className={`rounded-full border px-2.5 py-1 text-xs ${
                    selectedTags.includes(t.tag)
                      ? "tag-chip-active"
                      : "border-[var(--border)] text-[var(--muted)]"
                  }`}
                  onClick={() => toggleTag(t.tag)}
                >
                  {t.tag}
                  <span className="ml-1 opacity-70">{t.count}</span>
                </button>
              ))}
            </div>
          </section>
        )}
      </SideDrawer>

      <SideDrawer
        open={sortOpen}
        title="Sort library"
        onClose={() => setSortOpen(false)}
      >
        <div className="grid grid-cols-1 gap-1.5">
          {(
            [
              ["title_asc", "Title A–Z"],
              ["title_desc", "Title Z–A"],
              ["updated_desc", "Recently updated"],
              ["rating_desc", "F95 rating"],
              ["user_rating_desc", "Your rating"],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              className={`rounded-lg border px-3 py-2.5 text-left text-sm ${
                sort === value
                  ? "border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_16%,transparent)]"
                  : "border-[var(--border)] bg-[var(--bg-soft)]"
              }`}
              onClick={() => {
                setSort(value);
                setSortOpen(false);
              }}
            >
              {label}
            </button>
          ))}
        </div>
      </SideDrawer>
    </div>
  );
}
