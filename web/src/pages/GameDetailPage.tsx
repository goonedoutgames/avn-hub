import { FormEvent, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { Pencil } from "lucide-react";
import { FileUploadButton } from "@/components/FileUploadButton";
import { ScreenshotGallery } from "@/components/ScreenshotGallery";
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
  const [userRating, setUserRating] = useState("");
  const [displayTitle, setDisplayTitle] = useState("");
  const [editingTitle, setEditingTitle] = useState(false);
  const [patchDesc, setPatchDesc] = useState("");

  const load = async () => {
    setError(null);
    try {
      const d = await api.game(gameId);
      setDetail(d);
      setNotes(d.game.user_notes ?? "");
      setPlayStatus(d.game.play_status ?? "unplayed");
      setUserRating(d.game.user_rating != null ? String(d.game.user_rating) : "");
      setDisplayTitle(d.game.title);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load game");
    }
  };

  useEffect(() => {
    if (!Number.isFinite(gameId)) return;
    void load();
  }, [gameId]);

  if (!Number.isFinite(gameId)) {
    return <p className="text-[var(--danger)]">Invalid game id</p>;
  }

  const saveUserData = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      const d = await api.patchGame(gameId, {
        play_status: playStatus,
        user_notes: notes,
        user_rating: userRating === "" ? null : Number(userRating),
      });
      setDetail(d);
      toast.success("Saved");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Save failed";
      setError(msg);
      toast.error(msg);
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
      const d = await api.refreshGame(gameId);
      setDetail(d);
      setDisplayTitle(d.game.title);
      toast.success(
        d.game.title_custom
          ? "Metadata refreshed (custom title kept)"
          : "Metadata refreshed",
      );
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
                <img src={cover} alt="" className="h-full w-full object-cover" />
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
              {game.rating != null ? ` · ★ ${game.rating.toFixed(1)}` : ""}
            </p>
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
            <div className="mt-2 flex flex-wrap gap-1">
              {game.tags
                .filter((t) => !/^\d+$/.test(t))
                .map((t) => (
                  <span
                    key={t}
                    className="rounded-full border border-[var(--border)] px-2 py-0.5 text-xs text-[var(--muted)]"
                  >
                    {t}
                  </span>
                ))}
            </div>
          </div>

          {game.description && (
            <div className="card card-section whitespace-pre-wrap text-sm leading-relaxed">
              {game.description}
            </div>
          )}

          <form onSubmit={(e) => void saveUserData(e)} className="card card-section stack">
            <h2 className="m-0 text-base font-semibold">Your notes</h2>
            <div className="grid gap-3 sm:grid-cols-2">
              <label className="block text-sm">
                <span className="field-label">Play status</span>
                <select
                  className="input"
                  value={playStatus}
                  onChange={(e) => setPlayStatus(e.target.value)}
                >
                  <option value="unplayed">Unplayed</option>
                  <option value="playing">Playing</option>
                  <option value="completed">Completed</option>
                  <option value="dropped">Dropped</option>
                </select>
              </label>
              <label className="block text-sm">
                <span className="field-label">Your rating (0–5)</span>
                <input
                  className="input"
                  type="number"
                  min={0}
                  max={5}
                  step={0.5}
                  value={userRating}
                  onChange={(e) => setUserRating(e.target.value)}
                />
              </label>
            </div>
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

          {screenshots.length > 0 && (
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
          )}

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
        </div>
      </div>
    </div>
  );
}
