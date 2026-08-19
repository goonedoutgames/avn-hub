import { useEffect, useCallback, useState } from "react";
import { ChevronLeft, ChevronRight, Image as ImageIcon, Maximize2, X } from "lucide-react";
import { mediaUrl } from "@/lib/api";
import type { ScreenshotItem } from "@/lib/types";

type Props = {
  screenshots: ScreenshotItem[];
  isCustomCover?: boolean;
  busy?: boolean;
  onSetCover: (index: number) => void | Promise<void>;
  onResetCover?: () => void | Promise<void>;
};

/** Prefer hub-cached media; fall back to F95 source URL (same contract as Afterglow / OpenAPI). */
function shotSrc(s: ScreenshotItem): string | null {
  return mediaUrl(s.cached_url) ?? (s.full_url || null);
}

function canSetCover(s: ScreenshotItem): boolean {
  return Boolean(s.cached_url?.trim());
}

export function ScreenshotGallery({
  screenshots,
  isCustomCover,
  busy,
  onSetCover,
  onResetCover,
}: Props) {
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [openIdx, setOpenIdx] = useState<number | null>(null);
  const [broken, setBroken] = useState<Record<string, boolean>>({});

  const close = useCallback(() => setOpenIdx(null), []);
  const prev = useCallback(() => {
    setOpenIdx((i) => {
      if (i == null) return i;
      const nextIdx = (i + screenshots.length - 1) % screenshots.length;
      setSelectedIdx(nextIdx);
      return nextIdx;
    });
  }, [screenshots.length]);
  const next = useCallback(() => {
    setOpenIdx((i) => {
      if (i == null) return i;
      const nextIdx = (i + 1) % screenshots.length;
      setSelectedIdx(nextIdx);
      return nextIdx;
    });
  }, [screenshots.length]);

  useEffect(() => {
    setBroken({});
    setSelectedIdx(0);
    setOpenIdx(null);
  }, [screenshots]);

  useEffect(() => {
    if (selectedIdx >= screenshots.length) {
      setSelectedIdx(Math.max(0, screenshots.length - 1));
    }
  }, [screenshots.length, selectedIdx]);

  useEffect(() => {
    if (openIdx == null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
      if (e.key === "ArrowLeft") prev();
      if (e.key === "ArrowRight") next();
    };
    window.addEventListener("keydown", onKey);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = prevOverflow;
    };
  }, [openIdx, close, prev, next]);

  // When lightbox is closed, arrow keys change the inline selection.
  useEffect(() => {
    if (openIdx != null || screenshots.length === 0) return;
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setSelectedIdx((i) => (i + screenshots.length - 1) % screenshots.length);
      }
      if (e.key === "ArrowRight") {
        e.preventDefault();
        setSelectedIdx((i) => (i + 1) % screenshots.length);
      }
      if (e.key === "Enter") {
        e.preventDefault();
        setOpenIdx(selectedIdx);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openIdx, screenshots.length, selectedIdx]);

  if (screenshots.length === 0) {
    return (
      <div className="card card-section">
        <h2 className="m-0 text-base font-semibold">Screenshots</h2>
        <p className="muted mt-1 text-sm">
          No screenshots yet. Refresh metadata to pull the gallery onto the hub (and show F95
          stubs immediately).
        </p>
      </div>
    );
  }

  const safeSelected = Math.min(selectedIdx, screenshots.length - 1);
  const selected = screenshots[safeSelected];
  const active = openIdx != null ? screenshots[openIdx] : null;
  const resolveSrc = (s: ScreenshotItem, key: string) => {
    const preferred = shotSrc(s);
    if (broken[key] && s.full_url && preferred !== s.full_url) return s.full_url;
    return preferred;
  };
  const selectedSrc = resolveSrc(selected, `preview-${safeSelected}`);
  const activeSrc = active && openIdx != null ? resolveSrc(active, `full-${openIdx}`) : null;
  const cachedCount = screenshots.filter((s) => Boolean(s.cached_url?.trim())).length;

  return (
    <div className="stack">
      <div className="page-header">
        <div>
          <h2 className="m-0 text-base font-semibold">Screenshots</h2>
          <p className="page-subtitle sm:hidden">
            {screenshots.length} images · tap to enlarge
          </p>
          <p className="page-subtitle hidden sm:block">
            {screenshots.length} images
            {cachedCount > 0
              ? ` · ${cachedCount} cached on hub`
              : " · serving from F95 until cached"}
            {" · click the large image for fullscreen · GIFs play inline"}
          </p>
        </div>
        <div className="toolbar">
          {isCustomCover && onResetCover && (
            <button
              type="button"
              className="btn btn-sm"
              disabled={busy}
              onClick={() => void onResetCover()}
            >
              Reset cover
            </button>
          )}
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => setOpenIdx(safeSelected)}
          >
            <Maximize2 className="h-4 w-4" />
            Open gallery
          </button>
        </div>
      </div>

      <div className="gallery-carousel">
        <button
          type="button"
          className="gallery-preview"
          onClick={() => setOpenIdx(safeSelected)}
          aria-label={`Open screenshot ${safeSelected + 1} fullscreen`}
        >
          {selectedSrc ? (
            <img
              src={selectedSrc}
              alt=""
              referrerPolicy="no-referrer"
              onError={() => {
                if (selected.full_url && selectedSrc !== selected.full_url) {
                  setBroken((prev) => ({ ...prev, [`preview-${safeSelected}`]: true }));
                }
              }}
            />
          ) : (
            <span className="muted flex h-full items-center justify-center gap-2 text-sm">
              <ImageIcon className="h-5 w-5" /> Image unavailable
            </span>
          )}
          <span className="gallery-preview-hint">
            <Maximize2 className="h-3.5 w-3.5" />
            Click to enlarge
          </span>
        </button>

        <div className="gallery-strip" role="listbox" aria-label="Screenshot thumbnails">
          {screenshots.map((s, idx) => {
            const key = `thumb-${idx}`;
            const src = resolveSrc(s, key);
            return (
              <button
                key={idx}
                type="button"
                role="option"
                aria-selected={idx === safeSelected}
                className={`gallery-strip-item ${idx === safeSelected ? "is-active" : ""}`}
                onClick={() => setSelectedIdx(idx)}
                aria-label={`Select screenshot ${idx + 1}`}
              >
                {src ? (
                  <img
                    src={src}
                    alt=""
                    loading="lazy"
                    referrerPolicy="no-referrer"
                    onError={() => {
                      if (s.full_url && src !== s.full_url) {
                        setBroken((prev) => ({ ...prev, [key]: true }));
                      }
                    }}
                  />
                ) : (
                  <span className="muted flex h-full items-center justify-center text-[10px]">
                    —
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {openIdx != null && active && (
        <div
          className="lightbox"
          role="dialog"
          aria-modal="true"
          aria-label={`Screenshot ${openIdx + 1} of ${screenshots.length}`}
          onClick={close}
        >
          <div className="lightbox-toolbar" onClick={(e) => e.stopPropagation()}>
            <span className="text-sm font-medium">
              {openIdx + 1} / {screenshots.length}
            </span>
            <div className="toolbar">
              <button
                type="button"
                className="btn btn-sm btn-primary"
                disabled={busy || !canSetCover(active)}
                title={
                  canSetCover(active)
                    ? "Use this screenshot as the library cover"
                    : "Available after the hub caches this image"
                }
                onClick={() => void onSetCover(openIdx)}
              >
                Use as cover
              </button>
              <button type="button" className="btn btn-sm" onClick={close} aria-label="Close">
                <X className="h-4 w-4" />
              </button>
            </div>
          </div>

          <button
            type="button"
            className="lightbox-nav lightbox-nav-prev"
            onClick={(e) => {
              e.stopPropagation();
              prev();
            }}
            aria-label="Previous screenshot"
          >
            <ChevronLeft className="h-6 w-6" />
          </button>

          <div className="lightbox-stage" onClick={(e) => e.stopPropagation()}>
            {activeSrc ? (
              <img
                src={activeSrc}
                alt=""
                className="lightbox-image"
                referrerPolicy="no-referrer"
                onError={() => {
                  if (active.full_url && activeSrc !== active.full_url) {
                    setBroken((prev) => ({ ...prev, [`full-${openIdx}`]: true }));
                  }
                }}
              />
            ) : (
              <p className="muted">Image unavailable</p>
            )}
          </div>

          <button
            type="button"
            className="lightbox-nav lightbox-nav-next"
            onClick={(e) => {
              e.stopPropagation();
              next();
            }}
            aria-label="Next screenshot"
          >
            <ChevronRight className="h-6 w-6" />
          </button>

          <div className="lightbox-strip" onClick={(e) => e.stopPropagation()}>
            {screenshots.map((s, idx) => {
              const key = `strip-${idx}`;
              const src = resolveSrc(s, key);
              return (
                <button
                  key={idx}
                  type="button"
                  className={`lightbox-strip-item ${idx === openIdx ? "is-active" : ""}`}
                  onClick={() => {
                    setOpenIdx(idx);
                    setSelectedIdx(idx);
                  }}
                >
                  {src && (
                    <img
                      src={src}
                      alt=""
                      referrerPolicy="no-referrer"
                      onError={() => {
                        if (s.full_url && src !== s.full_url) {
                          setBroken((prev) => ({ ...prev, [key]: true }));
                        }
                      }}
                    />
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
