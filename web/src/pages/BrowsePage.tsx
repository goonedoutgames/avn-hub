import { FormEvent, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  ArrowDownAZ,
  CalendarClock,
  Eye,
  Link2,
  RefreshCw,
  SlidersHorizontal,
  Star,
  ThumbsUp,
} from "lucide-react";
import { CatalogGameCard } from "@/components/CatalogGameCard";
import { SideDrawer } from "@/components/SideDrawer";
import { humanTags } from "@/components/TagBadges";
import { useToast } from "@/context/ToastContext";
import { api } from "@/lib/api";
import type { F95SearchResult, LibraryTag } from "@/lib/types";

type SortKey = "date" | "likes" | "views" | "name" | "rating";
type SearchMode = "title" | "creator";

const SORTS: { key: SortKey; label: string; hint: string; icon: typeof Star }[] = [
  { key: "date", label: "Date", hint: "Recently updated", icon: CalendarClock },
  { key: "likes", label: "Likes", hint: "Most liked", icon: ThumbsUp },
  { key: "views", label: "Views", hint: "Most viewed", icon: Eye },
  { key: "name", label: "Name", hint: "Alphabetical", icon: ArrowDownAZ },
  { key: "rating", label: "Rating", hint: "Weighted rating", icon: Star },
];

const DATE_PRESETS: { days: number; label: string }[] = [
  { days: 0, label: "Any time" },
  { days: 7, label: "7 days" },
  { days: 30, label: "30 days" },
  { days: 90, label: "90 days" },
  { days: 180, label: "6 months" },
  { days: 365, label: "1 year" },
];

const ENGINES = ["", "Ren'Py", "Unity", "HTML", "RPGM", "VN", "Other"];

function dateLabel(days: number): string {
  const preset = DATE_PRESETS.find((p) => p.days === days);
  if (preset) return preset.label;
  if (days <= 0) return "Any time";
  if (days === 1) return "1 day";
  return `${days} days`;
}

function parseTagsParam(raw: string | null): string[] {
  if (!raw) return [];
  return raw
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
}

export function BrowsePage() {
  const navigate = useNavigate();
  const toast = useToast();
  const [searchParams, setSearchParams] = useSearchParams();
  const [query, setQuery] = useState("");
  const [searchMode, setSearchMode] = useState<SearchMode>("title");
  const [urlInput, setUrlInput] = useState("");
  const [page, setPage] = useState(1);
  const [sort, setSort] = useState<SortKey>("date");
  const [dateDays, setDateDays] = useState(0);
  const [results, setResults] = useState<F95SearchResult[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [adding, setAdding] = useState<number | null>(null);
  const [includeTags, setIncludeTags] = useState<string[]>(() =>
    parseTagsParam(searchParams.get("tags")),
  );
  const [excludeTags, setExcludeTags] = useState<string[]>([]);
  const [tagMode, setTagMode] = useState<"and" | "or">("and");
  const [engine, setEngine] = useState("");
  const [tagDraft, setTagDraft] = useState("");
  const [excludeDraft, setExcludeDraft] = useState("");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [libraryTags, setLibraryTags] = useState<LibraryTag[]>([]);

  useEffect(() => {
    const fromUrl = parseTagsParam(searchParams.get("tags"));
    setIncludeTags((prev) => {
      const same =
        prev.length === fromUrl.length && prev.every((t, i) => t === fromUrl[i]);
      return same ? prev : fromUrl;
    });
  }, [searchParams]);

  useEffect(() => {
    void api.libraryTags().then(setLibraryTags).catch(() => setLibraryTags([]));
  }, []);

  const syncIncludeToUrl = (next: string[]) => {
    const nextParams = new URLSearchParams(searchParams);
    if (next.length) nextParams.set("tags", next.join(","));
    else nextParams.delete("tags");
    setSearchParams(nextParams, { replace: true });
  };

  const runSearch = async (overrides?: {
    page?: number;
    sort?: SortKey;
    query?: string;
    searchMode?: SearchMode;
    dateDays?: number;
    tagMode?: "and" | "or";
    includeTags?: string[];
    excludeTags?: string[];
    engine?: string;
  }) => {
    const nextPage = overrides?.page ?? page;
    const nextSort = overrides?.sort ?? sort;
    const nextQuery = overrides?.query ?? query;
    const nextMode = overrides?.searchMode ?? searchMode;
    const nextDate = overrides?.dateDays ?? dateDays;
    const nextTagMode = overrides?.tagMode ?? tagMode;
    const nextInclude = overrides?.includeTags ?? includeTags;
    const nextExclude = overrides?.excludeTags ?? excludeTags;
    const nextEngine = overrides?.engine ?? engine;

    setBusy(true);
    setError(null);
    try {
      // F95 SAM accepts tag *names* (not the numeric IDs returned in list rows).
      const namedInclude = humanTags(nextInclude);
      const namedExclude = humanTags(nextExclude);
      const prefix =
        nextEngine && nextEngine.toLowerCase() !== "other" ? nextEngine : undefined;

      const list = await api.searchCatalog({
        q: nextMode === "title" ? nextQuery.trim() || undefined : undefined,
        creator: nextMode === "creator" ? nextQuery.trim() || undefined : undefined,
        page: nextPage,
        sort: nextSort,
        date: nextDate > 0 ? nextDate : undefined,
        tag_mode: nextTagMode,
        tags: namedInclude.length ? namedInclude.join(",") : undefined,
        notags: namedExclude.length ? namedExclude.join(",") : undefined,
        prefixes: prefix,
      });
      setResults(list);
      setPage(nextPage);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Search failed";
      setError(msg);
      toast.error(msg);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void runSearch({ page: 1 });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    sort,
    dateDays,
    tagMode,
    engine,
    includeTags.join("|"),
    excludeTags.join("|"),
  ]);

  // Named tags only — SAM list rows usually return numeric IDs which are useless as chips.
  const namedResultTags = useMemo(() => {
    const counts = new Map<string, number>();
    for (const r of results) {
      for (const t of humanTags(r.tags)) {
        counts.set(t, (counts.get(t) ?? 0) + 1);
      }
    }
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .map(([tag, count]) => ({ tag, count }));
  }, [results]);

  const filtered = useMemo(() => {
    if (!engine) return results;
    return results.filter((r) => {
      const prefixes = (r.prefixes ?? []).map((p) => p.toLowerCase());
      const eng = engine.toLowerCase();
      if (eng === "other") {
        const known = ["ren'py", "renpy", "unity", "html", "rpgm", "vn"];
        return !prefixes.some((p) => known.includes(p) || p.replace("'", "") === "renpy");
      }
      return prefixes.some(
        (p) => p === eng || p.replace("'", "") === eng.replace("'", ""),
      );
    });
  }, [results, engine]);

  const addByInput = async (input: string) => {
    setBusy(true);
    setError(null);
    try {
      toast.info("Adding game…");
      const detail = await api.addGame(input);
      toast.success(`Added ${detail.game.title}`);
      navigate(`/game/${detail.game.id}`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to add game";
      setError(msg);
      toast.error(msg);
    } finally {
      setBusy(false);
    }
  };

  const addResult = async (r: F95SearchResult) => {
    setAdding(r.thread_id);
    setError(null);
    try {
      toast.info(`Adding ${r.title}…`);
      const detail = await api.addGame(String(r.thread_id));
      toast.success(`Added ${detail.game.title}`);
      navigate(`/game/${detail.game.id}`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to add game";
      setError(msg);
      toast.error(msg);
    } finally {
      setAdding(null);
    }
  };

  const onUrlSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (urlInput.trim()) void addByInput(urlInput.trim());
  };

  const setInclude = (next: string[]) => {
    setIncludeTags(next);
    syncIncludeToUrl(next);
  };

  const addInclude = (tag: string) => {
    const t = tag.trim();
    if (!t || /^\d+$/.test(t) || includeTags.length >= 10) return;
    if (!includeTags.some((x) => x.toLowerCase() === t.toLowerCase())) {
      setInclude([...includeTags, t]);
    }
    setTagDraft("");
  };

  const addExclude = (tag: string) => {
    const t = tag.trim();
    if (!t || /^\d+$/.test(t) || excludeTags.length >= 10) return;
    if (!excludeTags.some((x) => x.toLowerCase() === t.toLowerCase())) {
      setExcludeTags([...excludeTags, t]);
    }
    setExcludeDraft("");
  };

  const clearFilters = () => {
    setIncludeTags([]);
    setExcludeTags([]);
    setEngine("");
    setQuery("");
    setDateDays(0);
    setSearchMode("title");
    setTagMode("and");
    syncIncludeToUrl([]);
    void runSearch({
      page: 1,
      query: "",
      dateDays: 0,
      includeTags: [],
      excludeTags: [],
      tagMode: "and",
      searchMode: "title",
      engine: "",
    });
  };

  const activeFilterCount = useMemo(() => {
    let n = 0;
    if (query.trim()) n += 1;
    if (dateDays > 0) n += 1;
    if (engine) n += 1;
    n += includeTags.length + excludeTags.length;
    if (tagMode === "or") n += 1;
    return n;
  }, [query, dateDays, engine, includeTags, excludeTags, tagMode]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Browse F95Zone</h1>
          <p className="page-subtitle">
            Discover games with the same sorts and filters as F95 latest updates.
          </p>
        </div>
        <button type="button" className="btn" disabled={busy} onClick={() => void runSearch()}>
          <RefreshCw className={`h-3.5 w-3.5 ${busy ? "animate-spin" : ""}`} />
          {busy ? "Loading…" : "Refresh"}
        </button>
      </div>

      <div className="toolbar">
        <button
          type="button"
          className="btn"
          disabled={busy || page <= 1}
          onClick={() => void runSearch({ page: page - 1 })}
        >
          Prev
        </button>
        {[page - 1, page, page + 1]
          .filter((p) => p >= 1)
          .filter((p, i, arr) => arr.indexOf(p) === i)
          .map((p) => (
            <button
              key={p}
              type="button"
              className={`btn min-w-10 ${p === page ? "btn-primary" : ""}`}
              disabled={busy}
              onClick={() => void runSearch({ page: p })}
            >
              {p}
            </button>
          ))}
        <button
          type="button"
          className="btn"
          disabled={busy}
          onClick={() => void runSearch({ page: page + 1 })}
        >
          Next
        </button>
        <span className="muted text-xs">
          {filtered.length} shown
          {dateDays > 0 ? ` · updated within ${dateLabel(dateDays).toLowerCase()}` : ""}
          {includeTags.length > 0 ? ` · tags: ${includeTags.join(", ")}` : ""}
        </span>
      </div>

      {error && <p className="text-sm text-[var(--danger)]">{error}</p>}

      {filtered.length === 0 && !busy ? (
        <p className="muted py-16 text-center">
          No games match these filters. Try clearing the date limit or tags.
        </p>
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {filtered.map((r) => (
            <CatalogGameCard
              key={r.thread_id}
              game={r}
              busy={adding === r.thread_id}
              onAdd={() => void addResult(r)}
            />
          ))}
        </div>
      )}

      <div className="fab-cluster">
        <button type="button" className="fab fab-primary" onClick={() => setDrawerOpen(true)}>
          <SlidersHorizontal className="h-4 w-4" />
          Search & filters
          {activeFilterCount > 0 && <span className="fab-badge">{activeFilterCount}</span>}
        </button>
      </div>

      <SideDrawer
        open={drawerOpen}
        title="Browse controls"
        subtitle="Search, filters, and add by URL"
        onClose={() => setDrawerOpen(false)}
        footer={
          <>
            <button type="button" className="btn" onClick={clearFilters}>
              Clear filters
            </button>
            <button
              type="button"
              className="btn btn-primary ml-auto"
              disabled={busy}
              onClick={() => {
                void runSearch({ page: 1 });
                setDrawerOpen(false);
              }}
            >
              Apply
            </button>
          </>
        }
      >
        <section className="stack">
          <div className="field-label">
            <Link2 className="mr-1 inline h-3.5 w-3.5" />
            Add from URL
          </div>
          <form onSubmit={onUrlSubmit} className="toolbar">
            <input
              className="input min-w-0 flex-1"
              placeholder="Paste F95 thread URL or id…"
              value={urlInput}
              onChange={(e) => setUrlInput(e.target.value)}
            />
            <button className="btn btn-primary shrink-0" type="submit" disabled={busy}>
              Add
            </button>
          </form>
        </section>

        <section className="stack">
          <div className="field-label">Sorting</div>
          <div className="grid grid-cols-1 gap-1.5">
            {SORTS.map((s) => {
              const Icon = s.icon;
              const active = sort === s.key;
              return (
                <button
                  key={s.key}
                  type="button"
                  className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition ${
                    active
                      ? "border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_16%,transparent)]"
                      : "border-[var(--border)] bg-[var(--bg-soft)] hover:border-[var(--accent-dim)]"
                  }`}
                  onClick={() => setSort(s.key)}
                >
                  <Icon
                    className={`h-4 w-4 shrink-0 ${active ? "text-[var(--accent)]" : "text-[var(--muted)]"}`}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm font-medium">{s.label}</span>
                    <span className="muted block text-[11px]">{s.hint}</span>
                  </span>
                </button>
              );
            })}
          </div>
        </section>

        <section className="stack">
          <div className="flex items-center justify-between gap-2">
            <div className="field-label mb-0">Date limit</div>
            <span className="text-xs text-[var(--text)]">{dateLabel(dateDays)}</span>
          </div>
          <input
            type="range"
            min={0}
            max={365}
            step={1}
            value={dateDays}
            onChange={(e) => setDateDays(Number(e.target.value))}
            className="w-full accent-[var(--accent)]"
          />
          <div className="flex flex-wrap gap-1.5">
            {DATE_PRESETS.map((p) => (
              <button
                key={p.days}
                type="button"
                className={`rounded-full border px-2.5 py-1 text-[11px] ${
                  dateDays === p.days
                    ? "border-[var(--accent)] text-[var(--text)]"
                    : "border-[var(--border)] text-[var(--muted)]"
                }`}
                onClick={() => setDateDays(p.days)}
              >
                {p.label}
              </button>
            ))}
          </div>
          <p className="muted text-[11px]">Only show games updated within this window.</p>
        </section>

        <section className="stack">
          <div className="flex items-center justify-between gap-2">
            <div className="field-label mb-0">Search</div>
            <div className="flex rounded-lg border border-[var(--border)] p-0.5">
              {(
                [
                  ["title", "Title"],
                  ["creator", "Creator"],
                ] as const
              ).map(([mode, label]) => (
                <button
                  key={mode}
                  type="button"
                  className={`rounded-md px-2.5 py-1 text-[11px] ${
                    searchMode === mode
                      ? "bg-[var(--accent)] font-semibold text-[#041018]"
                      : "text-[var(--muted)]"
                  }`}
                  onClick={() => setSearchMode(mode)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <div className="flex gap-2">
            <input
              className="input"
              placeholder={searchMode === "title" ? "Search titles…" : "Search creators…"}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void runSearch({ page: 1 });
              }}
            />
            <button
              type="button"
              className="btn btn-primary shrink-0"
              disabled={busy}
              onClick={() => void runSearch({ page: 1 })}
            >
              Go
            </button>
          </div>
        </section>

        <section className="stack">
          <div className="flex items-center justify-between gap-2">
            <div className="field-label mb-0">
              Include tags <span className="font-normal normal-case tracking-normal">(max 10)</span>
            </div>
            <button
              type="button"
              className="rounded-md border border-[var(--border)] px-2 py-1 text-[11px] font-semibold"
              onClick={() => setTagMode((m) => (m === "and" ? "or" : "and"))}
              title="Match all tags (AND) or any tag (OR)"
            >
              {tagMode.toUpperCase()}
            </button>
          </div>
          <p className="muted text-[11px]">
            Use F95 tag names (e.g. Fantasy, NTR) — not numeric IDs from the catalog API.
          </p>
          <div className="flex gap-2">
            <input
              className="input"
              list="browse-tags"
              placeholder="Type a tag name…"
              value={tagDraft}
              onChange={(e) => setTagDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addInclude(tagDraft);
                }
              }}
            />
            <button type="button" className="btn shrink-0" onClick={() => addInclude(tagDraft)}>
              Add
            </button>
          </div>
          {includeTags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {includeTags.map((t) => (
                <button
                  key={t}
                  type="button"
                  className="rounded-md bg-[var(--bg-soft)] px-2 py-1 text-[11px]"
                  onClick={() => setInclude(includeTags.filter((x) => x !== t))}
                >
                  {t} ×
                </button>
              ))}
            </div>
          )}
          {libraryTags.length > 0 && (
            <div className="max-h-36 space-y-1 overflow-y-auto rounded-lg border border-[var(--border)] p-2">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-[var(--muted)]">
                From your library
              </div>
              <p className="muted mb-2 text-[10px]">
                Tap to filter F95 by tags you already use. Numbers are how many library games have
                that tag.
              </p>
              <div className="flex flex-wrap gap-1">
                {libraryTags.slice(0, 40).map(({ tag, count }) => (
                  <button
                    key={tag}
                    type="button"
                    className="rounded border border-[var(--border)] px-1.5 py-0.5 text-[10px] text-[var(--muted)] hover:border-[var(--accent)] hover:text-[var(--text)]"
                    onClick={() => addInclude(tag)}
                    title={`${count} games in your library`}
                  >
                    {tag}
                    <span className="ml-1 opacity-60">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
          {namedResultTags.length > 0 && (
            <div className="max-h-28 space-y-1 overflow-y-auto rounded-lg border border-[var(--border)] p-2">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-[var(--muted)]">
                Named tags on this page
              </div>
              <p className="muted mb-2 text-[10px]">
                Quick-add tags that already appear as names on the current results.
              </p>
              <div className="flex flex-wrap gap-1">
                {namedResultTags.slice(0, 30).map(({ tag, count }) => (
                  <button
                    key={tag}
                    type="button"
                    className="rounded border border-[var(--border)] px-1.5 py-0.5 text-[10px] text-[var(--muted)] hover:border-[var(--accent)] hover:text-[var(--text)]"
                    onClick={() => addInclude(tag)}
                  >
                    {tag}
                    <span className="ml-1 opacity-60">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
          {libraryTags.length === 0 && namedResultTags.length === 0 && (
            <div className="hint-box">
              Catalog rows often only expose tag IDs. Type F95 tag names above, or add games to your
              library first to get quick-pick suggestions.
            </div>
          )}
        </section>

        <section className="stack">
          <div className="field-label">Exclude tags</div>
          <div className="flex gap-2">
            <input
              className="input"
              list="browse-tags"
              placeholder="Exclude tag name…"
              value={excludeDraft}
              onChange={(e) => setExcludeDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addExclude(excludeDraft);
                }
              }}
            />
            <button type="button" className="btn shrink-0" onClick={() => addExclude(excludeDraft)}>
              Add
            </button>
          </div>
          {excludeTags.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {excludeTags.map((t) => (
                <button
                  key={t}
                  type="button"
                  className="rounded-md bg-[color-mix(in_srgb,var(--danger)_22%,transparent)] px-2 py-1 text-[11px]"
                  onClick={() => setExcludeTags(excludeTags.filter((x) => x !== t))}
                >
                  {t} ×
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="stack">
          <div className="field-label">Engine / prefix</div>
          <select className="input" value={engine} onChange={(e) => setEngine(e.target.value)}>
            <option value="">Any</option>
            {ENGINES.filter(Boolean).map((e) => (
              <option key={e} value={e}>
                {e}
              </option>
            ))}
          </select>
          <p className="muted text-[11px]">
            Prefers engine labels from titles. “Other” filters this page only.
          </p>
        </section>

        <datalist id="browse-tags">
          {libraryTags.map(({ tag }) => (
            <option key={tag} value={tag} />
          ))}
          {namedResultTags.map(({ tag }) => (
            <option key={`r-${tag}`} value={tag} />
          ))}
        </datalist>
      </SideDrawer>
    </div>
  );
}
