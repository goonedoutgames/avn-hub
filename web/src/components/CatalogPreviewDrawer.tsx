import { useEffect, useMemo, useState } from "react";
import { Eye, ExternalLink, Star, ThumbsUp, X } from "lucide-react";
import { PlatformBadges } from "@/components/PlatformBadges";
import { SideDrawer } from "@/components/SideDrawer";
import { TagBadges } from "@/components/TagBadges";
import type { CatalogPreview } from "@/lib/types";

function formatCount(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 10_000 ? 0 : 1)}K`;
  return String(n);
}

function prefixClass(prefix: string): string {
  const p = prefix.toLowerCase();
  if (p === "vn") return "bg-[#c43c3c]";
  if (p.includes("ren")) return "bg-[#7b4bb8]";
  if (p === "unity") return "bg-[#c46a2b]";
  if (p === "html") return "bg-[#2f9e5f]";
  if (p === "rpgm") return "bg-[#3d7cc9]";
  if (p === "completed") return "bg-[#3d8fd4]";
  if (p === "abandoned") return "bg-[#c47a2b]";
  if (p.includes("hold")) return "bg-[#8a7a3a]";
  if (p === "cancelled") return "bg-[#666]";
  return "bg-[#4a5568]";
}

type Props = {
  open: boolean;
  preview: CatalogPreview | null;
  loading?: boolean;
  error?: string | null;
  adding?: boolean;
  onClose: () => void;
  onAdd: () => void;
  onOpenLibrary?: (gameId: number) => void;
};

export function CatalogPreviewDrawer({
  open,
  preview,
  loading,
  error,
  adding,
  onClose,
  onAdd,
  onOpenLibrary,
}: Props) {
  const [lightbox, setLightbox] = useState<number | null>(null);

  const gallery = useMemo(() => {
    if (!preview) return [];
    const urls = [preview.cover, ...(preview.screenshots ?? [])].filter(Boolean);
    const seen = new Set<string>();
    return urls.filter((u) => {
      const key = u.toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [preview]);

  useEffect(() => {
    setLightbox(null);
  }, [preview?.thread_id]);

  useEffect(() => {
    if (lightbox == null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setLightbox(null);
      if (e.key === "ArrowLeft") {
        setLightbox((i) =>
          i == null ? i : (i + gallery.length - 1) % gallery.length,
        );
      }
      if (e.key === "ArrowRight") {
        setLightbox((i) => (i == null ? i : (i + 1) % gallery.length));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [lightbox, gallery.length]);

  const subtitle = preview
    ? [preview.creator, preview.version ? `v${preview.version.replace(/^v/i, "")}` : null]
        .filter(Boolean)
        .join(" · ")
    : undefined;

  return (
    <>
      <SideDrawer
        open={open}
        title={preview?.title ?? (loading ? "Loading…" : "Game details")}
        subtitle={subtitle}
        onClose={onClose}
        side="right"
        footer={
          preview ? (
            <div className="flex flex-wrap gap-2">
              {preview.in_library && preview.library_game_id != null ? (
                <button
                  type="button"
                  className="btn btn-primary flex-1"
                  onClick={() => onOpenLibrary?.(preview.library_game_id!)}
                >
                  Open in library
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn-primary flex-1"
                  disabled={adding || loading}
                  onClick={onAdd}
                >
                  {adding ? "Adding…" : "Add to library"}
                </button>
              )}
              <a
                className="btn"
                href={preview.url}
                target="_blank"
                rel="noreferrer"
              >
                <ExternalLink className="mr-1 inline h-3.5 w-3.5" />
                F95
              </a>
            </div>
          ) : undefined
        }
      >
        {loading && !preview && (
          <p className="text-sm text-[var(--muted)]">Fetching overview and gallery from F95…</p>
        )}
        {error && <p className="text-sm text-[var(--danger)]">{error}</p>}
        {preview && (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-[var(--muted)]">
              {preview.date && <span>{preview.date}</span>}
              {preview.likes != null && (
                <span className="inline-flex items-center gap-1">
                  <ThumbsUp className="h-3 w-3" /> {formatCount(preview.likes)}
                </span>
              )}
              {preview.views != null && (
                <span className="inline-flex items-center gap-1">
                  <Eye className="h-3 w-3" /> {formatCount(preview.views)}
                </span>
              )}
              {preview.rating > 0 && (
                <span className="inline-flex items-center gap-1">
                  <Star className="h-3 w-3" /> {preview.rating.toFixed(1)}
                </span>
              )}
              {preview.in_library && (
                <span className="rounded bg-[var(--accent-dim)] px-1.5 py-0.5 text-[10px] font-semibold uppercase text-white">
                  In library
                </span>
              )}
            </div>

            {preview.prefixes?.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {preview.prefixes.map((p) => (
                  <span
                    key={p}
                    className={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-white ${prefixClass(p)}`}
                  >
                    {p}
                  </span>
                ))}
              </div>
            )}

            <PlatformBadges platforms={preview.platforms} />
            <TagBadges tags={preview.tags} limit={24} />

            {preview.description?.trim() ? (
              <div className="space-y-1">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                  Overview
                </h3>
                <p className="whitespace-pre-wrap text-sm leading-relaxed text-[var(--fg)]">
                  {preview.description.trim()}
                </p>
              </div>
            ) : (
              !loading && (
                <p className="text-sm text-[var(--muted)]">No overview text found on the thread.</p>
              )
            )}

            {gallery.length > 0 && (
              <div className="space-y-2">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
                  Gallery ({gallery.length})
                </h3>
                <div className="grid grid-cols-2 gap-2">
                  {gallery.map((src, i) => (
                    <button
                      key={`${src}-${i}`}
                      type="button"
                      className="overflow-hidden rounded-md border border-[var(--border)] bg-[var(--bg-soft)]"
                      onClick={() => setLightbox(i)}
                    >
                      <img
                        src={src}
                        alt=""
                        referrerPolicy="no-referrer"
                        className="aspect-video w-full object-cover"
                        loading="lazy"
                      />
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </SideDrawer>

      {lightbox != null && gallery[lightbox] && (
        <div
          className="fixed inset-0 z-[80] flex items-center justify-center bg-black/85 p-4"
          role="dialog"
          aria-modal="true"
          onClick={() => setLightbox(null)}
        >
          <button
            type="button"
            className="absolute right-4 top-4 btn"
            onClick={() => setLightbox(null)}
            aria-label="Close"
          >
            <X className="h-4 w-4" />
          </button>
          <img
            src={gallery[lightbox]}
            alt=""
            referrerPolicy="no-referrer"
            className="max-h-[90vh] max-w-[94vw] object-contain"
            onClick={(e) => e.stopPropagation()}
          />
          <div className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded bg-black/70 px-3 py-1 text-xs text-white">
            {lightbox + 1} / {gallery.length}
          </div>
        </div>
      )}
    </>
  );
}
