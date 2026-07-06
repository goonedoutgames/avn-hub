import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  Download,
  ExternalLink,
  Link2,
  Star,
  Trash2,
  Unlink,
} from "lucide-react";
import { api } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import { bestImageUrl } from "@/lib/image-url";
import type { GameDetail } from "@/lib/types";
import { decodeHtmlEntities, formatBytes } from "@/lib/utils";
import { ArchiveUpload } from "@/components/ArchiveUpload";
import { ScreenshotGallery } from "@/components/ScreenshotGallery";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export function GameDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { runTask } = useTasks();
  const [detail, setDetail] = useState<GameDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    setError(null);
    try {
      const data = await runTask(
        `game-detail-${id}`,
        "Loading game details",
        async (update) => {
          update("Fetching metadata and media…");
          return api.getGameDetail(Number(id));
        },
      );
      setDetail(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load game");
    } finally {
      setLoading(false);
    }
  }, [id, runTask]);

  useEffect(() => {
    load();
  }, [load]);

  const handleUnmatch = async () => {
    if (!detail || !confirm("Remove metadata match for this archive?")) return;
    try {
      await runTask(
        `unmatch-${detail.game.id}`,
        "Removing match",
        async () => {
          await api.unmatchGame(detail.game.id);
        },
      );
      navigate("/match");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Unmatch failed");
    }
  };

  const handleDownload = async () => {
    if (!detail) return;
    try {
      await runTask(
        `download-${detail.game.id}`,
        `Downloading ${detail.game.title}`,
        async (update) => {
          update(`Preparing ${detail.game.archive_filename}…`);
          await api.downloadGame(detail.game.id, detail.game.archive_filename);
        },
      );
    } catch (e) {
      alert(e instanceof Error ? e.message : "Download failed");
    }
  };

  const handleDeleteArchive = async () => {
    if (!detail) return;
    const { game } = detail;
    if (
      !confirm(
        `Delete “${game.archive_filename}” and remove all metadata? This cannot be undone.`,
      )
    ) {
      return;
    }
    try {
      await runTask(
        `delete-archive-${game.id}`,
        `Deleting ${game.archive_filename}`,
        async () => api.deleteArchive(game.id),
      );
      navigate("/match");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Delete failed");
    }
  };

  const handleSetCover = async (screenshotIndex: number) => {
    if (!detail) return;
    try {
      await runTask(
        `set-cover-${detail.game.id}`,
        "Updating cover",
        async () => api.setGameCover(detail.game.id, screenshotIndex),
      );
      const refreshed = await api.getGameDetail(detail.game.id);
      setDetail(refreshed);
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to set cover");
    }
  };

  if (loading) {
    return <p className="text-[var(--color-muted-foreground)]">Loading…</p>;
  }

  if (error || !detail) {
    return (
      <div className="space-y-4">
        <p className="text-red-300">{error ?? "Game not found"}</p>
        <Button variant="secondary" asChild>
          <Link to="/">
            <ArrowLeft className="h-4 w-4" />
            Back to library
          </Link>
        </Button>
      </div>
    );
  }

  const { game, cover_url, cover_full_url, screenshots } = detail;
  const title = decodeHtmlEntities(game.title);
  const coverDisplayUrl = bestImageUrl(cover_url, cover_full_url);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/">
            <ArrowLeft className="h-4 w-4" />
            Library
          </Link>
        </Button>
        <div className="flex flex-1 flex-wrap gap-2">
          <Button size="sm" onClick={handleDownload}>
            <Download className="h-4 w-4" />
            Download archive
          </Button>
          <Button size="sm" variant="secondary" asChild>
            <Link
              to={`/match?archive=${encodeURIComponent(game.archive_path)}`}
            >
              <Link2 className="h-4 w-4" />
              Re-match
            </Link>
          </Button>
          <Button size="sm" variant="outline" onClick={handleUnmatch}>
            <Unlink className="h-4 w-4" />
            Unmatch
          </Button>
          <ArchiveUpload
            replaceGameId={game.id}
            variant="outline"
            onComplete={load}
          />
          <Button size="sm" variant="destructive" onClick={handleDeleteArchive}>
            <Trash2 className="h-4 w-4" />
            Delete archive
          </Button>
          {game.f95_url && (
            <Button size="sm" variant="outline" asChild>
              <a href={game.f95_url} target="_blank" rel="noreferrer">
                <ExternalLink className="h-4 w-4" />
                F95Zone
              </a>
            </Button>
          )}
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-[minmax(280px,420px)_1fr]">
        <div className="space-y-4">
          <Card className="overflow-hidden">
            <div className="aspect-video bg-[var(--color-muted)]">
              {coverDisplayUrl ? (
                <img
                  key={coverDisplayUrl}
                  src={coverDisplayUrl}
                  alt={title}
                  className="h-full w-full object-cover"
                />
              ) : (
                <div className="flex h-full items-center justify-center text-sm text-[var(--color-muted-foreground)]">
                  No cover
                </div>
              )}
            </div>
          </Card>

          <div className="space-y-3">
            <div>
              <h1 className="text-2xl font-bold">{title}</h1>
              {game.developer && (
                <p className="text-[var(--color-muted-foreground)]">
                  {decodeHtmlEntities(game.developer)}
                </p>
              )}
            </div>

            <div className="flex flex-wrap gap-x-3 gap-y-1 text-sm text-[var(--color-muted-foreground)]">
              {game.version && <span>Version: {game.version}</span>}
              {game.rating != null && game.rating > 0 && (
                <span className="flex items-center gap-1">
                  <Star className="h-4 w-4 fill-yellow-400 text-yellow-400" />
                  {game.rating.toFixed(1)}
                </span>
              )}
              <span>{formatBytes(game.archive_size)}</span>
            </div>
            <p className="truncate text-xs text-[var(--color-muted-foreground)]">
              {game.archive_filename}
            </p>
          </div>
        </div>

        <div className="space-y-4">
          {game.description && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Description</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="max-h-none whitespace-pre-wrap text-sm leading-relaxed text-[var(--color-muted-foreground)]">
                  {decodeHtmlEntities(game.description)}
                </div>
              </CardContent>
            </Card>
          )}

          {game.tags.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Tags</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-wrap gap-2">
                  {game.tags.map((tag) => (
                    <span
                      key={tag}
                      className="rounded-full bg-[var(--color-secondary)] px-2.5 py-1 text-xs"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          {screenshots.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Screenshots</CardTitle>
                <CardDescription>
                  Browse screenshots or set one as the cover image
                </CardDescription>
              </CardHeader>
              <CardContent>
                <ScreenshotGallery
                  screenshots={screenshots}
                  coverUrl={cover_url}
                  onSetCover={handleSetCover}
                />
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
