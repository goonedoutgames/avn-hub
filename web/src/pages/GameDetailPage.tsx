import { FormEvent, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { Pencil } from "lucide-react";
import { FileUploadButton } from "@/components/FileUploadButton";
import { PlatformBadges } from "@/components/PlatformBadges";
import { ScreenshotGallery } from "@/components/ScreenshotGallery";
import { PLAY_STATUSES, PlayStatusBadge, StarRating } from "@/components/StarRating";
import { TagBadges } from "@/components/TagBadges";
import { useToast } from "@/context/ToastContext";
import { api, formatBytes, getStoredToken, mediaUrl, resolveApiBase } from "@/lib/api";
import type { GameDetail, VersionCheckResult } from "@/lib/types";

export function GameDetailPage() {
  const { id } = useParams();
  const gameId = Number(id);
  const navigate = useNavigate();
  const toast = useToast();
  const [detail, setDetail] = useState<GameDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [version, setVersion] = useState<VersionCheckResult | null>(null);
  const [notes, setNotes] = useState("");
  const [playStatus, setPlayStatus] = useState("unplayed");
  const [userRating, setUserRating] = useState<number | null>(null);
  const [displayTitle, setDisplayTitle] = useState("");
  const [editingTitle, setEditingTitle] = useState(false);
  const [patchDesc, setPatchDesc] = useState("");
  const [tagClickAction, setTagClickAction] = useState<"library" | "browse">("library");

  const load = async () => {
    setError(null);
    try {
      const [d, settings] = await Promise.all([api.game(gameId), api.settings()]);
      setDetail(d);
      setNotes(d.game.user_notes ?? "");
      setPlayStatus(d.game.play_status ?? "unplayed");
      setUserRating(d.game.user_rating);
      setDisplayTitle(d.game.title);
      setTagClickAction(settings.tag_click_action === "browse" ? "browse" : "library");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load game");
    }
  };

  const onTagClick = (tag: string) => {
    const q = encodeURIComponent(tag);
    if (tagClickAction === "browse") {
      navigate(`/browse?tags=${q}`);
    } else {
      navigate(`/?tags=${q}`);
    }
  };

  useEffect(() => {
    if (!Number.isFinite(gameId)) return;
    void load();
  }, [gameId]);

  if (!Number.isFinite(gameId)) {
    return <p className="text-[var(--danger)]">Invalid game id</p>;
  }

  const saveNotes = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      const d = await api.patchGame(gameId, { user_notes: notes });
      setDetail(d);
      toast.success("Notes saved");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Save failed";
      setError(msg);
      toast.error(msg);
    } finally {
      setBusy(false);
    }
  };

  const savePlayStatus = async (status: string) => {
    setPlayStatus(status);
    setBusy(true);
    try {
      const d = await api.patchGame(gameId, { play_status: status });
      setDetail(d);
      setPlayStatus(d.game.play_status ?? status);
      toast.success("Status updated");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update status");
      await load();
    } finally {
      setBusy(false);
    }
  };

  const saveUserRating = async (rating: number | null) => {
    setUserRating(rating);
    setBusy(true);
    try {
      const d = await api.patchGame(gameId, { user_rating: rating });
      setDetail(d);
      setUserRating(d.game.user_rating);
      toast.success(rating == null ? "Rating cleared" : "Rating saved");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update rating");
      await load();
    } finally {
      setBusy(false);
    }
  };

  const saveTitleOnly = async () => {
    const trimmed = displayTitle.trim();
    if (!trimmed) {
      toast.error("Title cannot be empty");
      return;
    }
    setBusy(true);
    try {
      const d = await api.patchGame(gameId, { title: trimmed });
      setDetail(d);
      setDisplayTitle(d.game.title);
      setEditingTitle(false);
      toast.success("Display title updated");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to update title";
      toast.error(msg);
    } finally {
      setBusy(false);
    }
  };

  const resetTitle = async () => {
    setBusy(true);
    try {
      const d = await api.patchGame(gameId, { reset_title: true });
      setDetail(d);
      setDisplayTitle(d.game.title);
      setEditingTitle(false);
      toast.success("Restored catalog title");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to reset title");
    } finally {
      setBusy(false);
    }
  };

  const onCheckVersion = async () => {
    setBusy(true);
    try {
      const result = await api.checkVersion(gameId);
      setVersion(result);
      toast.success(
        result.update_available
          ? `Update available: ${result.latest_version || "newer version"}`
          : "Up to date",
      );
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Version check failed");
    } finally {
      setBusy(false);
    }
  };

  const onRefresh = async () => {
    setBusy(true);
    try {
      toast.info("Refreshing metadata and caching screenshots…");
      const d = await api.refreshGame(gameId);
      setDetail(d);
      setDisplayTitle(d.game.title);
      setPlayStatus(d.game.play_status ?? "unplayed");
      setUserRating(d.game.user_rating);
      const cached = d.screenshots.filter((s) => Boolean(s.cached_url?.trim())).length;
      const total = d.screenshots.length;
      const titleNote = d.game.title_custom ? " (custom title kept)" : "";
      if (total === 0) {
        toast.success(`Metadata refreshed${titleNote}`);
      } else if (cached > 0) {
        toast.success(`Metadata refreshed${titleNote} · ${cached}/${total} screenshots cached on hub`);
      } else {
        toast.success(
          `Metadata refreshed${titleNote} · ${total} screenshots listed (hub still caching)`,
        );
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Refresh failed");
    } finally {
      setBusy(false);
    }
  };

  const onDelete = async () => {
    if (!confirm("Remove this game from your library?")) return;
    try {
      await api.deleteGame(gameId);
      toast.success("Removed from library");
      navigate("/");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Delete failed");
    }
  };

  const downloadWithAuth = async (url: string, filename: string) => {
    const base = await resolveApiBase();
    const full = url.startsWith("http") ? url : `${base}${url}`;
    const res = await fetch(full, {
      headers: { Authorization: `Bearer ${getStoredToken() ?? ""}` },
    });
    if (!res.ok) throw new Error("Download failed");
    const blob = await res.blob();
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = filename;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  if (!detail && !error) {
    return <p className="muted">Loading…</p>;
  }
  if (!detail) {
    return <p className="text-[var(--danger)]">{error}</p>;
  }

  const { game, screenshots, saves, patches } = detail;
  const cover = mediaUrl(detail.cover_url) ?? detail.cover_full_url;
  const sourceTitle = game.source_title?.trim() || null;

  return (
    <div className="page">
      <Link to="/" className="muted text-sm hover:text-[var(--text)]">
        ← Library
      </Link>

      {error && <p className="text-sm text-[var(--danger)]">{error}</p>}

      <div className="grid gap-6 lg:grid-cols-[minmax(200px,260px)_minmax(0,1fr)]">
        <div className="stack">
          <div className="card overflow-hidden">
            <div className="aspect-[16/10] bg-[var(--bg-soft)] sm:aspect-[3/4]">
              {cover ? (
                <img
                  src={cover}
                  alt=""
                  referrerPolicy="no-referrer"
                  className="h-full w-full object-cover"
                />
              ) : (
                <div className="muted flex h-full items-center justify-center p-4 text-center text-sm">
                  No cover
                </div>
              )}
            </div>
          </div>
          <div className="flex flex-col gap-2">
            {game.f95_url && (
              <a className="btn btn-primary" href={game.f95_url} target="_blank" rel="noreferrer">
                Open on F95Zone
              </a>
            )}
            <button type="button" className="btn" disabled={busy} onClick={() => void onCheckVersion()}>
              Check version
            </button>
            <button type="button" className="btn" disabled={busy} onClick={() => void onRefresh()}>
              Refresh metadata
            </button>
            <button type="button" className="btn btn-danger" onClick={() => void onDelete()}>
              Remove from library
            </button>
          </div>
          {version && (
            <div className="card card-section text-sm">
              <div>Stored: {version.stored_version ?? "—"}</div>
              <div>Latest: {version.latest_version || "—"}</div>
              <div className={version.update_available ? "text-[var(--warning)]" : "text-[var(--ok)]"}>
                {version.update_available ? "Update available" : "Up to date"}
              </div>
            </div>
          )}
        </div>

        <div className="flex flex-col gap-5">
          <div>
            {editingTitle ? (
              <div className="stack">
                <label className="block text-sm">
                  <span className="field-label">Display title</span>
                  <input
                    className="input text-lg font-semibold"
                    value={displayTitle}
                    autoFocus
                    onChange={(e) => setDisplayTitle(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        void saveTitleOnly();
                      }
                      if (e.key === "Escape") {
                        setDisplayTitle(game.title);
                        setEditingTitle(false);
                      }
                    }}
                  />
                </label>
                <div className="toolbar">
                  <button
                    type="button"
                    className="btn btn-primary btn-sm"
                    disabled={busy}
                    onClick={() => void saveTitleOnly()}
                  >
                    Save title
                  </button>
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={busy}
                    onClick={() => {
                      setDisplayTitle(game.title);
                      setEditingTitle(false);
                    }}
                  >
                    Cancel
                  </button>
                  {game.title_custom && (
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={busy}
                      onClick={() => void resetTitle()}
                    >
                      Restore catalog title
                    </button>
                  )}
                </div>
              </div>
            ) : (
              <div className="flex flex-wrap items-start gap-2">
                <h1 className="page-title">{game.title}</h1>
                <button
                  type="button"
                  className="btn btn-sm btn-ghost"
                  title="Edit display title"
                  onClick={() => {
                    setDisplayTitle(game.title);
                    setEditingTitle(true);
                  }}
                >
                  <Pencil className="h-3.5 w-3.5" />
                  Edit
                </button>
              </div>
            )}
            <p className="page-subtitle">
              {game.developer ?? "Unknown"}
              {game.version ? ` · v${game.version}` : ""}
            </p>
            <PlatformBadges platforms={game.platforms} size="md" className="mt-2" />
            <div className="meta-chip-row mt-2">
              <PlayStatusBadge status={playStatus} size="md" />
            </div>
            <div className="rating-row mt-3">
              <StarRating
                label="F95"
                value={game.rating != null && game.rating > 0 ? game.rating : null}
                size="md"
                showValue
              />
              <StarRating
                label="Yours"
                value={userRating}
                size="md"
                showValue
                disabled={busy}
                onChange={(v) => void saveUserRating(v)}
              />
            </div>
            {game.title_custom && sourceTitle && sourceTitle !== game.title && !editingTitle && (
              <p className="muted mt-1 text-xs">
                Catalog title: {sourceTitle}{" "}
                <button
                  type="button"
                  className="text-[var(--accent)] underline-offset-2 hover:underline"
                  disabled={busy}
                  onClick={() => void resetTitle()}
                >
                  restore
                </button>
              </p>
            )}
            <TagBadges
              className="mt-2"
              tags={game.tags}
              limit={5}
              size="md"
              onTagClick={onTagClick}
            />
          </div>

          {game.description && (
            <div className="card card-section whitespace-pre-wrap text-sm leading-relaxed">
              {game.description}
            </div>
          )}

          <section className="card card-section stack">
            <div>
              <h2 className="m-0 text-base font-semibold">Play status</h2>
              <p className="muted mt-1 text-xs">Track where you are with this game.</p>
            </div>
            <div className="status-pills">
              {PLAY_STATUSES.map((s) => (
                <button
                  key={s.value}
                  type="button"
                  className={`status-pill status-pill-${s.value} ${playStatus === s.value ? "is-active" : ""}`}
                  disabled={busy}
                  onClick={() => void savePlayStatus(s.value)}
                >
                  {s.label}
                </button>
              ))}
            </div>
          </section>

          <form onSubmit={(e) => void saveNotes(e)} className="card card-section stack">
            <h2 className="m-0 text-base font-semibold">Your notes</h2>
            <textarea
              className="input min-h-28"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Review or notes…"
            />
            <button className="btn btn-primary self-start" type="submit" disabled={busy}>
              Save notes
            </button>
          </form>

          <div className="grid gap-4 md:grid-cols-2">
            <section className="card card-section stack">
              <div>
                <h2 className="m-0 text-base font-semibold">Saves</h2>
                <p className="muted mt-1 text-xs">Back up small save files for this game.</p>
              </div>
              <FileUploadButton
                label="Upload save"
                hint="Click Browse or drop a save file"
                onFile={async (file) => {
                  try {
                    await api.uploadSave(gameId, file);
                    await load();
                    toast.success(`Uploaded ${file.name}`);
                  } catch (err) {
                    toast.error(err instanceof Error ? err.message : "Upload failed");
                  }
                }}
              />
              <ul className="m-0 flex list-none flex-col gap-2 p-0">
                {saves.map((s) => (
                  <li key={s.id} className="file-row">
                    <span className="min-w-0 truncate text-sm">
                      {s.filename} <span className="muted">({formatBytes(s.size)})</span>
                    </span>
                    <span className="toolbar">
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={() =>
                          void api
                            .downloadSaveUrl(gameId, s.id)
                            .then((url) => downloadWithAuth(url, s.filename))
                            .then(() => toast.success("Download started"))
                            .catch((err) =>
                              toast.error(err instanceof Error ? err.message : "Download failed"),
                            )
                        }
                      >
                        Download
                      </button>
                      <button
                        type="button"
                        className="btn btn-sm btn-danger"
                        onClick={() =>
                          void api
                            .deleteSave(gameId, s.id)
                            .then(() => load())
                            .then(() => toast.success("Save deleted"))
                            .catch((err) =>
                              toast.error(err instanceof Error ? err.message : "Delete failed"),
                            )
                        }
                      >
                        Delete
                      </button>
                    </span>
                  </li>
                ))}
                {saves.length === 0 && <li className="muted text-sm">No saves yet.</li>}
              </ul>
            </section>

            <section className="card card-section stack">
              <div>
                <h2 className="m-0 text-base font-semibold">Patches</h2>
                <p className="muted mt-1 text-xs">Store small patch files with an optional note.</p>
              </div>
              <label className="block text-sm">
                <span className="field-label">Description (optional)</span>
                <input
                  className="input"
                  placeholder="e.g. Hotfix for v0.4"
                  value={patchDesc}
                  onChange={(e) => setPatchDesc(e.target.value)}
                />
              </label>
              <FileUploadButton
                label="Upload patch"
                hint="Click Browse or drop a patch file"
                onFile={async (file) => {
                  try {
                    await api.uploadPatch(gameId, file, patchDesc || undefined);
                    setPatchDesc("");
                    await load();
                    toast.success(`Uploaded ${file.name}`);
                  } catch (err) {
                    toast.error(err instanceof Error ? err.message : "Upload failed");
                  }
                }}
              />
              <ul className="m-0 flex list-none flex-col gap-2 p-0">
                {patches.map((p) => (
                  <li key={p.id} className="file-row">
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm">
                        {p.filename} <span className="muted">({formatBytes(p.size)})</span>
                      </div>
                      {p.description && (
                        <p className="muted m-0 mt-0.5 text-xs">{p.description}</p>
                      )}
                    </div>
                    <span className="toolbar">
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={() =>
                          void api
                            .downloadPatchUrl(gameId, p.id)
                            .then((url) => downloadWithAuth(url, p.filename))
                            .then(() => toast.success("Download started"))
                            .catch((err) =>
                              toast.error(err instanceof Error ? err.message : "Download failed"),
                            )
                        }
                      >
                        Download
                      </button>
                      <button
                        type="button"
                        className="btn btn-sm btn-danger"
                        onClick={() =>
                          void api
                            .deletePatch(gameId, p.id)
                            .then(() => load())
                            .then(() => toast.success("Patch deleted"))
                            .catch((err) =>
                              toast.error(err instanceof Error ? err.message : "Delete failed"),
                            )
                        }
                      >
                        Delete
                      </button>
                    </span>
                  </li>
                ))}
                {patches.length === 0 && <li className="muted text-sm">No patches yet.</li>}
              </ul>
            </section>
          </div>

          <ScreenshotGallery
            screenshots={screenshots}
            isCustomCover={detail.is_custom_cover}
            busy={busy}
            onSetCover={async (idx) => {
              setBusy(true);
              try {
                setDetail(await api.setCover(gameId, idx));
                toast.success("Cover updated");
              } catch (err) {
                toast.error(err instanceof Error ? err.message : "Failed to set cover");
              } finally {
                setBusy(false);
              }
            }}
            onResetCover={async () => {
              setBusy(true);
              try {
                setDetail(await api.resetCover(gameId));
                toast.success("Cover reset");
              } catch (err) {
                toast.error(err instanceof Error ? err.message : "Failed to reset cover");
              } finally {
                setBusy(false);
              }
            }}
          />
        </div>
      </div>
    </div>
  );
}
