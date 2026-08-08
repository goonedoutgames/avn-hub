import { useEffect, useCallback, useState } from "react";
import { ChevronLeft, ChevronRight, Image as ImageIcon, X } from "lucide-react";
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
  const [openIdx, setOpenIdx] = useState<number | null>(null);
  const [broken, setBroken] = useState<Record<string, boolean>>({});

  const close = useCallback(() => setOpenIdx(null), []);
  const prev = useCallback(() => {
    setOpenIdx((i) => (i == null ? i : (i + screenshots.length - 1) % screenshots.length));
  }, [screenshots.length]);
  const next = useCallback(() => {
    setOpenIdx((i) => (i == null ? i : (i + 1) % screenshots.length));
  }, [screenshots.length]);

  useEffect(() => {
    setBroken({});
  }, [screenshots]);

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

  const active = openIdx != null ? screenshots[openIdx] : null;
  const resolveSrc = (s: ScreenshotItem, key: string) => {
    const preferred = shotSrc(s);
    if (broken[key] && s.full_url && preferred !== s.full_url) return s.full_url;
    return preferred;
  };
  const activeSrc = active && openIdx != null ? resolveSrc(active, `full-${openIdx}`) : null;
  const cachedCount = screenshots.filter((s) => Boolean(s.cached_url?.trim())).length;

  return (
    <div className="stack">
      <div className="page-header">
        <div>
          <h2 className="m-0 text-base font-semibold">Screenshots</h2>
          <p className="page-subtitle">
            {screenshots.length} images
            {cachedCount > 0
              ? ` · ${cachedCount} cached on hub`
              : " · serving from F95 until cached"}
            {" · click to enlarge · GIFs play in the lightbox"}
          </p>
        </div>
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
      </div>

      <div className="gallery-grid">
        {screenshots.map((s, idx) => {
          const key = `thumb-${idx}`;
          const src = resolveSrc(s, key);
          return (
            <button
              key={idx}
              type="button"
              className="gallery-thumb"
              onClick={() => setOpenIdx(idx)}
              aria-label={`Open screenshot ${idx + 1}`}
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
                <span className="muted flex h-full items-center justify-center gap-1 text-xs">
                  <ImageIcon className="h-4 w-4" /> Missing
                </span>
              )}
              <span className="gallery-thumb-index">{idx + 1}</span>
            </button>
          );
        })}
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
                  onClick={() => setOpenIdx(idx)}
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
