import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Download, ExternalLink, Filter, Search, Star } from "lucide-react";
import { api } from "@/lib/api";
import { bestImageUrl } from "@/lib/image-url";
import { useTasks } from "@/context/TaskContext";
import type { GameResponse, LibraryTag, TagFilterMode } from "@/lib/types";
import { decodeHtmlEntities, formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CoverPreview } from "@/components/CoverPreview";
import { TagFilterPanel } from "@/components/TagFilterBox";
import { FloatingSheetButton, Sheet } from "@/components/ui/sheet";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const MAX_VISIBLE_TAGS = 3;

function serializeTags(tags: string[]): string | undefined {
  if (tags.length === 0) return undefined;
  return tags.join(",");
}

export function LibraryPage() {
  const { runTask } = useTasks();
  const [games, setGames] = useState<GameResponse[]>([]);
  const [availableTags, setAvailableTags] = useState<LibraryTag[]>([]);
  const [nameSearch, setNameSearch] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [tagMode, setTagMode] = useState<TagFilterMode>("and");
  const [searchOpen, setSearchOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (
      name?: string,
      tags?: string[],
      mode: TagFilterMode = "and",
    ) => {
      setLoading(true);
      setError(null);
      try {
        const results = await runTask(
          "library-load",
          "Loading library",
          async (update) => {
            update("Fetching matched games…");
            const list = await api.listGames(
              name,
              serializeTags(tags ?? []),
              mode,
            );
            return list.filter((g) => g.game?.matched);
          },
        );
        setGames(results);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to load library");
      } finally {
        setLoading(false);
      }
    },
    [runTask],
  );

  const loadTags = useCallback(async () => {
    try {
      const tags = await api.listLibraryTags();
      setAvailableTags(tags);
    } catch {
      setAvailableTags([]);
    }
  }, []);

  useEffect(() => {
    loadTags();
  }, [loadTags]);

  useEffect(() => {
    load(nameSearch, selectedTags, tagMode);
  }, [selectedTags, tagMode]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleNameSearch = (e: React.FormEvent) => {
    e.preventDefault();
    load(nameSearch, selectedTags, tagMode);
    setSearchOpen(false);
  };

  const addTagFilter = (tag: string) => {
    const key = tag.toLowerCase();
    if (selectedTags.some((t) => t.toLowerCase() === key)) return;
    setSelectedTags((prev) => [...prev, tag]);
    setFilterOpen(true);
  };

  const clearFilters = () => {
    setNameSearch("");
    setSelectedTags([]);
    setTagMode("and");
    load();
  };

  const clearTagFilters = () => {
    setSelectedTags([]);
    setTagMode("and");
  };

  const hasNameSearch = nameSearch.trim().length > 0;
  const hasTagFilters = selectedTags.length > 0;
  const hasFilters = hasNameSearch || hasTagFilters;

  return (
    <div className="space-y-6">
      <FloatingSheetButton
        side="left"
        icon={<Search className="h-5 w-5" />}
        label="Search library"
        active={hasNameSearch}
        onClick={() => setSearchOpen(true)}
      />
      <FloatingSheetButton
        side="right"
        icon={<Filter className="h-5 w-5" />}
        label="Filter by tags"
        active={hasTagFilters}
        badge={hasTagFilters ? selectedTags.length : undefined}
        onClick={() => setFilterOpen(true)}
      />

      <Sheet
        open={searchOpen}
        onOpenChange={setSearchOpen}
        side="left"
        title="Search"
        description="Match titles and developers — separate from tag filters."
      >
        <form onSubmit={handleNameSearch} className="space-y-4">
          <Input
            placeholder="Search by title or developer…"
            value={nameSearch}
            onChange={(e) => setNameSearch(e.target.value)}
            autoFocus
          />
          <div className="flex gap-2">
            <Button type="submit" className="flex-1" disabled={loading}>
              <Search className="h-4 w-4" />
              Search
            </Button>
            {hasNameSearch && (
              <Button
                type="button"
                variant="outline"
                onClick={() => {
                  setNameSearch("");
                  load("", selectedTags, tagMode);
                }}
                disabled={loading}
              >
                Clear
              </Button>
            )}
          </div>
        </form>
      </Sheet>

      <Sheet
        open={filterOpen}
        onOpenChange={setFilterOpen}
        side="right"
        title="Filter by tags"
        description="What kinks and fetishes should we play today? Pick from your library — combine with AND or OR."
      >
        <TagFilterPanel
          availableTags={availableTags}
          selectedTags={selectedTags}
          mode={tagMode}
          onSelectedTagsChange={setSelectedTags}
          onModeChange={setTagMode}
          onClear={clearTagFilters}
          disabled={loading}
        />
      </Sheet>

      <div>
        <h1 className="text-2xl font-bold">Library</h1>
        <p className="text-sm text-[var(--color-muted-foreground)]">
          Browse your matched visual novel collection
          {!loading && games.length > 0 && ` · ${games.length} games`}
          {!loading && hasFilters && games.length === 0 && " · no matches"}
        </p>
        {hasFilters && (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            {hasNameSearch && (
              <button
                type="button"
                onClick={() => setSearchOpen(true)}
                className="inline-flex items-center gap-1 rounded-full border border-[var(--color-border)] bg-[var(--color-secondary)] px-2.5 py-0.5 text-xs transition-colors hover:border-[var(--color-primary)]"
              >
                <Search className="h-3 w-3" />
                {nameSearch}
              </button>
            )}
            {selectedTags.map((tag) => (
              <button
                key={tag}
                type="button"
                onClick={() => setFilterOpen(true)}
                className="rounded-full border border-[var(--color-primary)]/40 bg-[var(--color-primary)]/15 px-2.5 py-0.5 text-xs transition-colors hover:bg-[var(--color-primary)]/25"
              >
                {tag}
              </button>
            ))}
            {tagMode === "or" && hasTagFilters && (
              <span className="text-[10px] uppercase tracking-wide text-[var(--color-muted-foreground)]">
                OR
              </span>
            )}
            <button
              type="button"
              onClick={clearFilters}
              className="text-xs text-[var(--color-muted-foreground)] underline-offset-2 hover:underline"
            >
              Clear all
            </button>
          </div>
        )}
      </div>

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          {error}
        </div>
      )}

      {loading && games.length === 0 ? (
        <p className="text-[var(--color-muted-foreground)]">Loading library…</p>
      ) : games.length === 0 && hasFilters ? (
        <Card>
          <CardHeader>
            <CardTitle>No games match your filters</CardTitle>
            <CardDescription>
              Try fewer tags, switch to OR mode, or use the side panels to
              adjust search and filters.
            </CardDescription>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className="mt-2 w-fit"
              onClick={clearFilters}
            >
              Clear all filters
            </Button>
          </CardHeader>
        </Card>
      ) : games.length === 0 ? (
        <Card>
          <CardHeader>
            <CardTitle>No matched games yet</CardTitle>
            <CardDescription>
              Configure your archive folder in Settings, scan for archives, then
              match them to F95Zone metadata.
            </CardDescription>
          </CardHeader>
        </Card>
      ) : (
        <div className="grid auto-rows-fr gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {games.map((entry) => (
            <GameCard
              key={entry.game.id}
              entry={entry}
              onTagClick={addTagFilter}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function GameCard({
  entry,
  onTagClick,
}: {
  entry: GameResponse;
  onTagClick?: (tag: string) => void;
}) {
  const { game, cover_url, cover_full_url, preview_urls = [] } = entry;
  const coverDisplayUrl = bestImageUrl(cover_url, cover_full_url);
  const { runTask } = useTasks();
  const [downloading, setDownloading] = useState(false);
  const title = decodeHtmlEntities(game.title);
  const developer = game.developer
    ? decodeHtmlEntities(game.developer)
    : null;

  const handleDownload = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDownloading(true);
    try {
      await runTask(
        `download-${game.id}`,
        `Downloading ${title}`,
        async (update) => {
          update(`Preparing ${game.archive_filename}…`);
          await api.downloadGame(game.id, game.archive_filename);
        },
      );
    } catch (err) {
      alert(err instanceof Error ? err.message : "Download failed");
    } finally {
      setDownloading(false);
    }
  };

  return (
    <Link to={`/game/${game.id}`} className="block h-full">
      <Card className="flex h-full flex-col overflow-hidden transition-colors hover:border-[var(--color-primary)]">
        <div className="aspect-video shrink-0 bg-[var(--color-muted)]">
          <CoverPreview
            coverUrl={coverDisplayUrl}
            previewUrls={preview_urls}
            alt={title}
          />
        </div>
        <CardContent className="flex flex-1 flex-col p-4">
          <div className="min-h-[2.75rem]">
            <h3 className="line-clamp-2 font-semibold leading-tight">{title}</h3>
          </div>
          <p className="mt-1 h-4 truncate text-xs text-[var(--color-muted-foreground)]">
            {developer ?? "\u00A0"}
          </p>

          <div className="mt-2 flex h-4 items-center gap-2 text-xs text-[var(--color-muted-foreground)]">
            {game.version ? (
              <span className="truncate">v{game.version}</span>
            ) : (
              <span>&nbsp;</span>
            )}
            {game.rating != null && game.rating > 0 && (
              <span className="flex shrink-0 items-center gap-1">
                <Star className="h-3 w-3 fill-yellow-400 text-yellow-400" />
                {game.rating.toFixed(1)}
              </span>
            )}
            <span className="ml-auto shrink-0">
              {formatBytes(game.archive_size)}
            </span>
          </div>

          <div className="mt-2 h-10 overflow-hidden">
            {game.tags.length > 0 ? (
              <div className="flex flex-wrap items-center gap-1">
                {game.tags.slice(0, MAX_VISIBLE_TAGS).map((tag) => (
                  <button
                    key={tag}
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      onTagClick?.(tag);
                    }}
                    className="rounded-full bg-[var(--color-secondary)] px-2 py-0.5 text-[10px] transition-colors hover:bg-[var(--color-primary)] hover:text-[var(--color-primary-foreground)]"
                  >
                    {tag}
                  </button>
                ))}
                {game.tags.length > MAX_VISIBLE_TAGS && (
                  <span className="rounded-full bg-[var(--color-accent)] px-2 py-0.5 text-[10px] text-[var(--color-muted-foreground)]">
                    +{game.tags.length - MAX_VISIBLE_TAGS} more
                  </span>
                )}
              </div>
            ) : null}
          </div>

          <div className="mt-auto flex gap-2 pt-3">
            <Button
              size="sm"
              className="flex-1"
              onClick={handleDownload}
              disabled={downloading}
            >
              <Download className="h-3 w-3" />
              {downloading ? "…" : "Download"}
            </Button>
            {game.f95_url && (
              <Button
                size="sm"
                variant="outline"
                onClick={(e) => e.stopPropagation()}
                asChild
              >
                <a href={game.f95_url} target="_blank" rel="noreferrer">
                  <ExternalLink className="h-3 w-3" />
                </a>
              </Button>
            )}
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}
