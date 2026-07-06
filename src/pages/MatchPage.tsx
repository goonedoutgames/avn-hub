import { useCallback, useEffect, useRef, useState } from "react";
import { Link2, RefreshCw, Search, Trash2, Unlink } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { ArchiveUpload } from "@/components/ArchiveUpload";
import { api } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import type { ArchiveEntry, F95SearchResult } from "@/lib/types";
import { formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const MATCH_STEPS = [
  { at: 0, detail: "Fetching F95Zone thread metadata…", progress: 15 },
  { at: 2000, detail: "Downloading cover image…", progress: 40 },
  { at: 5000, detail: "Downloading screenshots…", progress: 65 },
  { at: 9000, detail: "Saving to library…", progress: 85 },
];

function MatchCoverThumb({ result }: { result: F95SearchResult }) {
  const candidates = [
    result.cover,
    ...result.screenshots,
  ].filter((url) => url.trim().length > 0);
  const [index, setIndex] = useState(0);
  const src = candidates[index];

  if (!src) {
    return (
      <div className="flex h-16 w-12 items-center justify-center rounded bg-[var(--color-muted)] text-[10px]">
        N/A
      </div>
    );
  }

  return (
    <img
      src={src}
      alt=""
      className="h-16 w-12 rounded object-cover"
      referrerPolicy="no-referrer"
      onError={() => {
        setIndex((i) => (i + 1 < candidates.length ? i + 1 : i));
      }}
    />
  );
}

function MatchResultRow({
  result,
  matching,
  onMatch,
}: {
  result: F95SearchResult;
  matching: boolean;
  onMatch: () => void;
}) {
  return (
    <div className="flex gap-3 rounded-lg border border-[var(--color-border)] p-3">
      <MatchCoverThumb result={result} />
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{result.title}</p>
        <p className="text-xs text-[var(--color-muted-foreground)]">
          {result.creator}
          {result.version && ` · v${result.version}`}
        </p>
        <Button
          size="sm"
          className="mt-2"
          onClick={onMatch}
          disabled={matching}
        >
          <Link2 className="h-3 w-3" />
          {matching ? "Matching…" : "Match"}
        </Button>
      </div>
    </div>
  );
}

export function MatchPage() {
  const { runTask } = useTasks();
  const [searchParams] = useSearchParams();
  const [archives, setArchives] = useState<ArchiveEntry[]>([]);
  const [selected, setSelected] = useState<ArchiveEntry | null>(null);
  const [suggestions, setSuggestions] = useState<F95SearchResult[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [suggestedQuery, setSuggestedQuery] = useState("");
  const [searchResults, setSearchResults] = useState<F95SearchResult[]>([]);
  const [loadingArchives, setLoadingArchives] = useState(false);
  const [loadingSuggestions, setLoadingSuggestions] = useState(false);
  const [searching, setSearching] = useState(false);
  const [matching, setMatching] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [f95Url, setF95Url] = useState("");
  const [urlResult, setUrlResult] = useState<F95SearchResult | null>(null);
  const [resolvingUrl, setResolvingUrl] = useState(false);

  const loadArchives = useCallback(async () => {
    setLoadingArchives(true);
    try {
      setArchives(await api.listArchives());
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Failed to load archives");
    } finally {
      setLoadingArchives(false);
    }
  }, []);

  useEffect(() => {
    loadArchives();
  }, [loadArchives]);

  useEffect(() => {
    const archivePath = searchParams.get("archive");
    if (!archivePath || archives.length === 0) return;
    const archive = archives.find((a) => a.path === archivePath);
    if (archive) {
      selectArchive(archive);
    }
  }, [searchParams, archives]);

  const handleScan = async () => {
    setMessage(null);
    try {
      const result = await runTask(
        "scan-archives",
        "Scanning archive folder",
        async (update) => {
          update("Looking for .zip, .rar, .7z, .bz2 files…", 20);
          const scan = await api.scanArchives();
          update(
            `Found ${scan.total} archives (${scan.added} new)`,
            100,
          );
          return scan;
        },
      );
      setMessage(
        `Scan complete: ${result.added} new, ${result.updated} updated (${result.total} total archives)`,
      );
      await loadArchives();
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Scan failed");
    }
  };

  const selectArchive = async (archive: ArchiveEntry) => {
    setSelected(archive);
    setSearchResults([]);
    setUrlResult(null);
    setF95Url("");
    setMessage(null);
    setLoadingSuggestions(true);
    try {
      const results = await runTask(
        `suggest-${archive.path}`,
        "Finding F95Zone matches",
        async (update) => {
          update(`Parsing "${archive.filename}"…`, 20);
          const found = await api.suggestMatches(archive.path);
          update(`Found ${found.length} suggestions`, 100);
          return found;
        },
      );
      setSuggestions(results);
      const guess = archive.filename
        .replace(/\.(tar\.)?(bz2|rar|zip|7z)$/i, "")
        .replace(/[\u2018\u2019`´]/g, "'")
        .replace(/[_\.-]/g, " ")
        .replace(/\s+/g, " ")
        .trim();
      setSuggestedQuery(guess);
      setSearchQuery(guess);
    } catch (e) {
      setSuggestions([]);
      setMessage(e instanceof Error ? e.message : "Failed to get suggestions");
    } finally {
      setLoadingSuggestions(false);
    }
  };

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim()) return;
    setSearching(true);
    setUrlResult(null);
    try {
      const results = await runTask(
        "f95-search",
        "Searching F95Zone",
        async (update) => {
          update(`Query: "${searchQuery.trim()}"`);
          return api.searchF95(searchQuery);
        },
      );
      setSearchResults(results);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Search failed");
    } finally {
      setSearching(false);
    }
  };

  const handleResolveUrl = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!f95Url.trim() || !selected) return;
    setResolvingUrl(true);
    setMessage(null);
    setSearchResults([]);
    try {
      const result = await runTask(
        "f95-url",
        "Looking up F95 thread",
        async (update) => {
          update("Fetching thread metadata…");
          return api.resolveF95Thread(f95Url.trim());
        },
      );
      setUrlResult(result);
    } catch (e) {
      setUrlResult(null);
      setMessage(e instanceof Error ? e.message : "Could not resolve F95 URL");
    } finally {
      setResolvingUrl(false);
    }
  };

  const stepTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const handleMatch = async (threadId: number, hint?: F95SearchResult) => {
    if (!selected) return;
    setMatching(true);
    setMessage(null);

    try {
      const matched = await runTask(
        "match-archive",
        `Matching ${selected.filename}`,
        async (update) => {
          let stepIdx = 0;
          update(MATCH_STEPS[0].detail, MATCH_STEPS[0].progress);

          stepTimerRef.current = setInterval(() => {
            stepIdx = Math.min(stepIdx + 1, MATCH_STEPS.length - 1);
            const step = MATCH_STEPS[stepIdx];
            update(step.detail, step.progress);
          }, 2500);

          try {
            return await api.matchArchive({
              archive_path: selected.path,
              thread_id: threadId,
              hint,
            });
          } finally {
            if (stepTimerRef.current) {
              clearInterval(stepTimerRef.current);
              stepTimerRef.current = null;
            }
          }
        },
        "Starting match…",
      );

      setMessage(
        `Matched "${selected.filename}" as "${matched.game.title}"`,
      );
      await loadArchives();
      setSelected(null);
      setSuggestions([]);
      setSearchResults([]);
      setUrlResult(null);
      setF95Url("");
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Match failed");
    } finally {
      setMatching(false);
    }
  };

  const handleDeleteArchive = async (archive: ArchiveEntry) => {
    if (!archive.game_id) return;
    const prompt = archive.matched
      ? `Delete “${archive.filename}” and remove all metadata? This cannot be undone.`
      : `Delete “${archive.filename}”? This cannot be undone.`;
    if (!confirm(prompt)) return;
    try {
      await runTask(
        `delete-archive-${archive.game_id}`,
        `Deleting ${archive.filename}`,
        async () => api.deleteArchive(archive.game_id!),
      );
      if (selected?.path === archive.path) {
        setSelected(null);
        setSuggestions([]);
        setSearchResults([]);
        setUrlResult(null);
        setF95Url("");
      }
      await loadArchives();
      setMessage(`Deleted ${archive.filename}`);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Delete failed");
    }
  };

  const unmatched = archives.filter((a) => !a.matched);
  const matched = archives.filter((a) => a.matched);
  const baseResults =
    searchResults.length > 0 ? searchResults : suggestions;
  const results = urlResult
    ? baseResults.filter((r) => r.thread_id !== urlResult.thread_id)
    : baseResults;

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold">Match Archives</h1>
          <p className="text-sm text-[var(--color-muted-foreground)]">
            Link local archive files to F95Zone metadata
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={handleScan}>
            <RefreshCw className="h-4 w-4" />
            Scan Archives
          </Button>
          <ArchiveUpload onComplete={loadArchives} />
        </div>
      </div>

      {message && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-secondary)] px-4 py-3 text-sm">
          {message}
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Unmatched Archives</CardTitle>
            <CardDescription>
              {loadingArchives
                ? "Loading archives…"
                : `${unmatched.length} of ${archives.length} archives need metadata`}
            </CardDescription>
          </CardHeader>
          <CardContent className="max-h-[32rem] space-y-2 overflow-y-auto">
            {loadingArchives && archives.length === 0 ? (
              <p className="text-sm text-[var(--color-muted-foreground)]">
                Loading archives…
              </p>
            ) : unmatched.length === 0 ? (
              <p className="text-sm text-[var(--color-muted-foreground)]">
                All archives are matched. Run a scan to find new files.
              </p>
            ) : (
              unmatched.map((archive) => (
                <div
                  key={archive.path}
                  className={`flex items-center gap-2 rounded-lg border p-2 transition-colors ${
                    selected?.path === archive.path
                      ? "border-[var(--color-primary)] bg-[var(--color-accent)]"
                      : "border-[var(--color-border)]"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => selectArchive(archive)}
                    disabled={loadingSuggestions || matching}
                    className="min-w-0 flex-1 rounded-md p-1 text-left hover:bg-[var(--color-accent)]/50 disabled:opacity-50"
                  >
                    <p className="truncate font-medium">{archive.filename}</p>
                    <p className="text-xs text-[var(--color-muted-foreground)]">
                      {formatBytes(archive.size)}
                    </p>
                  </button>
                  {archive.game_id && (
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => handleDeleteArchive(archive)}
                      disabled={matching}
                      aria-label={`Delete ${archive.filename}`}
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  )}
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>
              {selected ? `Match: ${selected.filename}` : "Select an archive"}
            </CardTitle>
            <CardDescription>
              {loadingSuggestions
                ? "Searching F95Zone for suggestions…"
                : "Search F95Zone, paste a thread link, or use suggested matches"}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {selected && (
              <>
                {suggestedQuery && suggestions.length > 0 && (
                  <p className="text-xs text-[var(--color-muted-foreground)]">
                    Suggested from filename: &ldquo;{suggestedQuery}&rdquo;
                  </p>
                )}
                <form onSubmit={handleSearch} className="flex gap-2">
                  <Input
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="Search F95Zone..."
                    disabled={matching}
                  />
                  <Button
                    type="submit"
                    variant="secondary"
                    disabled={searching || matching}
                  >
                    <Search className="h-4 w-4" />
                  </Button>
                </form>
                <div className="border-t border-[var(--color-border)] pt-4">
                  <p className="mb-2 text-xs text-[var(--color-muted-foreground)]">
                    Can&apos;t find the game? Paste an F95 thread link:
                  </p>
                  <form onSubmit={handleResolveUrl} className="flex gap-2">
                    <Input
                      value={f95Url}
                      onChange={(e) => setF95Url(e.target.value)}
                      placeholder="https://f95zone.to/threads/..."
                      disabled={matching || resolvingUrl}
                    />
                    <Button
                      type="submit"
                      variant="secondary"
                      disabled={resolvingUrl || matching || !f95Url.trim()}
                    >
                      <Link2 className="h-4 w-4" />
                    </Button>
                  </form>
                </div>
              </>
            )}

            <div className="max-h-[24rem] space-y-2 overflow-y-auto">
              {!selected ? (
                <p className="text-sm text-[var(--color-muted-foreground)]">
                  Select an archive from the left to begin matching.
                </p>
              ) : loadingSuggestions ? (
                <p className="text-sm text-[var(--color-muted-foreground)]">
                  Loading suggestions…
                </p>
              ) : (
                <>
                  {urlResult && (
                    <MatchResultRow
                      result={urlResult}
                      matching={matching}
                      onMatch={() =>
                        handleMatch(urlResult.thread_id, urlResult)
                      }
                    />
                  )}
                  {results.length === 0 && !urlResult ? (
                    <p className="text-sm text-[var(--color-muted-foreground)]">
                      No search results. Try a different term or paste an F95
                      thread URL above.
                    </p>
                  ) : (
                    results.map((result) => (
                      <MatchResultRow
                        key={result.thread_id}
                        result={result}
                        matching={matching}
                        onMatch={() =>
                          handleMatch(result.thread_id, result)
                        }
                      />
                    ))
                  )}
                </>
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {matched.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Matched Archives</CardTitle>
            <CardDescription>
              Re-match or unmatch to fix incorrect metadata
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-2 sm:grid-cols-2">
            {matched.map((archive) => (
              <div
                key={archive.path}
                className="flex items-center justify-between gap-2 rounded-lg border border-[var(--color-border)] p-3"
              >
                <p className="truncate text-sm font-medium">
                  {archive.filename}
                </p>
                <div className="flex shrink-0 gap-1">
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => selectArchive(archive)}
                  >
                    <Link2 className="h-3 w-3" />
                    Re-match
                  </Button>
                  {archive.game_id && (
                    <>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleDeleteArchive(archive)}
                        aria-label={`Delete ${archive.filename}`}
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={async () => {
                          if (!confirm("Unmatch this archive?")) return;
                          try {
                            await api.unmatchGame(archive.game_id!);
                            await loadArchives();
                            setMessage(`Unmatched ${archive.filename}`);
                          } catch (e) {
                            setMessage(
                              e instanceof Error ? e.message : "Unmatch failed",
                            );
                          }
                        }}
                      >
                        <Unlink className="h-3 w-3" />
                      </Button>
                    </>
                  )}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
