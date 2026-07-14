import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  Download,
  ExternalLink,
  ImageOff,
  Link2,
  RefreshCw,
  Star,
  Trash2,
  Unlink,
} from "lucide-react";
import { api } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import { bestImageUrl } from "@/lib/image-url";
import type { GameDetail, VersionCheckResult } from "@/lib/types";
import { decodeHtmlEntities, formatBytes } from "@/lib/utils";
import { GameUserNotesCard } from "@/components/GameUserNotesCard";
import { GameFilesCard } from "@/components/GameFilesCard";
import { ResponsiveActions } from "@/components/MobileActionMenu";
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
  const [versionCheck, setVersionCheck] = useState<VersionCheckResult | null>(null);
  const [checkingVersion, setCheckingVersion] = useState(false);

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
      await runTask(`unmatch-${detail.game.id}`, "Removing match", async () => {
        await api.unmatchGame(detail.game.id);
      });
      navigate("/match");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Unmatch failed");
    }
  };

  const handleCheckVersion = async () => {
    if (!detail) return;
    setCheckingVersion(true);
    setVersionCheck(null);
    try {
      const result = await runTask(
        `check-version-${detail.game.id}`,
        "Checking F95Zone for updates",
        async (update) => {
          update("Fetching latest thread version…");
          return api.checkGameVersion(detail.game.id);
        },
      );
      setVersionCheck(result);
    } catch (e) {
      alert(e instanceof Error ? e.message : "Version check failed");
    } finally {
      setCheckingVersion(false);
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
      await runTask(`set-cover-${detail.game.id}`, "Updating cover", async () =>
        api.setGameCover(detail.game.id, screenshotIndex),
      );
      setDetail(await api.getGameDetail(detail.game.id));
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to set cover");
    }
  };

  const handleResetCover = async () => {
    if (!detail) return;
    try {
      await runTask(`reset-cover-${detail.game.id}`, "Resetting cover", async () =>
        api.resetGameCover(detail.game.id),
      );
      setDetail(await api.getGameDetail(detail.game.id));
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to reset cover");
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

  const { game, cover_url, cover_full_url, screenshots, is_custom_cover, attachments } =
    detail;
  const title = decodeHtmlEntities(game.title);
  const coverDisplayUrl = bestImageUrl(cover_url, cover_full_url);

  const actionItems = [
    {
      key: "download",
      label: "Download archive",
      icon: <Download className="h-4 w-4" />,
      onClick: handleDownload,
    },
    {
      key: "rematch",
      label: "Re-match",
      icon: <Link2 className="h-4 w-4" />,
      onClick: () =>
        navigate(
          `/match?archive_id=${attachments.platform_archives.find((a) => a.is_default)?.id ?? attachments.platform_archives[0]?.id ?? ""}&archive=${encodeURIComponent(game.archive_path)}`,
        ),
    },
    {
      key: "unmatch",
      label: "Unmatch",
      icon: <Unlink className="h-4 w-4" />,
      onClick: handleUnmatch,
      variant: "outline" as const,
    },
    {
      key: "delete",
      label: "Delete archive",
      icon: <Trash2 className="h-4 w-4" />,
      onClick: handleDeleteArchive,
      variant: "destructive" as const,
    },
    {
      key: "check-version",
      label: "Check for update",
      icon: <RefreshCw className="h-4 w-4" />,
      onClick: handleCheckVersion,
      hidden: !game.matched || !game.f95_thread_id,
      variant: "outline" as const,
    },
    {
      key: "f95",
      label: "Open on F95Zone",
      icon: <ExternalLink className="h-4 w-4" />,
      onClick: () => game.f95_url && window.open(game.f95_url, "_blank"),
      hidden: !game.f95_url,
      variant: "outline" as const,
    },
  ];

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/">
            <ArrowLeft className="h-4 w-4" />
            <span className="hidden sm:inline">Library</span>
          </Link>
        </Button>
        <ResponsiveActions menuLabel="Game actions" menuItems={actionItems}>
          <Button size="sm" onClick={handleDownload}>
            <Download className="h-4 w-4" />
            Download
          </Button>
          <Button size="sm" variant="secondary" asChild>
            <Link
              to={`/match?archive_id=${attachments.platform_archives.find((a) => a.is_default)?.id ?? ""}&archive=${encodeURIComponent(game.archive_path)}`}
            >
              <Link2 className="h-4 w-4" />
              Re-match
            </Link>
          </Button>
          <Button size="sm" variant="outline" onClick={handleUnmatch}>
            <Unlink className="h-4 w-4" />
            Unmatch
          </Button>
          <Button size="sm" variant="destructive" onClick={handleDeleteArchive}>
            <Trash2 className="h-4 w-4" />
            Delete
          </Button>
          {game.f95_url && (
            <Button size="sm" variant="outline" asChild>
              <a href={game.f95_url} target="_blank" rel="noreferrer">
                <ExternalLink className="h-4 w-4" />
                F95Zone
              </a>
            </Button>
          )}
        </ResponsiveActions>
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
            {is_custom_cover && (
              <div className="border-t border-[var(--color-border)] px-4 py-2">
                <Button
                  size="sm"
                  variant="outline"
                  className="w-full sm:w-auto"
                  onClick={handleResetCover}
                >
                  <ImageOff className="h-3 w-3" />
                  Reset to default cover
                </Button>
              </div>
            )}
          </Card>

          <div className="space-y-3">
            <div>
              <h1 className="text-xl font-bold sm:text-2xl">{title}</h1>
              {game.developer && (
                <p className="text-[var(--color-muted-foreground)]">
                  {decodeHtmlEntities(game.developer)}
                </p>
              )}
            </div>

            <div className="flex flex-wrap gap-x-3 gap-y-1 text-sm text-[var(--color-muted-foreground)]">
              {game.version && <span>Your version: {game.version}</span>}
              {game.rating != null && game.rating > 0 && (
                <span className="flex items-center gap-1">
                  <Star className="h-4 w-4 fill-yellow-400 text-yellow-400" />
                  {game.rating.toFixed(1)} community
                </span>
              )}
              {game.user_rating != null && game.user_rating > 0 && (
                <span className="flex items-center gap-1">
                  <Star className="h-4 w-4 fill-blue-400 text-blue-400" />
                  {game.user_rating.toFixed(0)} yours
                </span>
              )}
              <span>{formatBytes(game.archive_size)}</span>
            </div>
            <p className="truncate text-xs text-[var(--color-muted-foreground)]">
              {game.archive_filename}
            </p>

            {game.matched && game.f95_thread_id && (
              <div className="space-y-2">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleCheckVersion}
                  disabled={checkingVersion}
                  className="w-full sm:w-auto"
                >
                  <RefreshCw className={`h-3 w-3 ${checkingVersion ? "animate-spin" : ""}`} />
                  {checkingVersion ? "Checking F95Zone…" : "Check for update"}
                </Button>
                {versionCheck && (
                  <div
                    className={`rounded-lg border px-3 py-2 text-sm ${
                      versionCheck.update_available
                        ? "border-amber-500/40 bg-amber-500/10"
                        : "border-[var(--color-border)] bg-[var(--color-secondary)]"
                    }`}
                  >
                    {versionCheck.update_available ? (
                      <>
                        <p className="font-medium text-amber-100">
                          Update likely available
                        </p>
                        <p className="mt-1 text-[var(--color-muted-foreground)]">
                          Your library:{" "}
                          <strong>{versionCheck.stored_version ?? "unknown"}</strong>
                          {" · "}
                          F95Zone: <strong>{versionCheck.latest_version}</strong>
                        </p>
                        {versionCheck.f95_url && (
                          <a
                            href={versionCheck.f95_url}
                            target="_blank"
                            rel="noreferrer"
                            className="mt-2 inline-flex items-center gap-1 text-xs underline underline-offset-2"
                          >
                            <ExternalLink className="h-3 w-3" />
                            View thread on F95Zone
                          </a>
                        )}
                      </>
                    ) : (
                      <p className="text-[var(--color-muted-foreground)]">
                        Up to date with F95Zone
                        {versionCheck.latest_version
                          ? ` (v${versionCheck.latest_version})`
                          : ""}
                        .
                      </p>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>

          <GameUserNotesCard
            game={game}
            onUpdated={(updated) =>
              setDetail((d) => (d ? { ...d, game: updated } : d))
            }
          />
        </div>

        <div className="space-y-4">
          <GameFilesCard
            gameId={game.id}
            attachments={attachments}
            onUpdated={load}
          />

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
