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

function fullSrc(s: ScreenshotItem): string | null {
  return s.full_url || mediaUrl(s.cached_url) || null;
}

function thumbSrc(s: ScreenshotItem): string | null {
  return mediaUrl(s.cached_url) ?? (s.full_url || null);
}

export function ScreenshotGallery({
  screenshots,
  isCustomCover,
  busy,
  onSetCover,
  onResetCover,
}: Props) {
  const [openIdx, setOpenIdx] = useState<number | null>(null);

  const close = useCallback(() => setOpenIdx(null), []);
  const prev = useCallback(() => {
    setOpenIdx((i) => (i == null ? i : (i + screenshots.length - 1) % screenshots.length));
  }, [screenshots.length]);
  const next = useCallback(() => {
    setOpenIdx((i) => (i == null ? i : (i + 1) % screenshots.length));
  }, [screenshots.length]);

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

  if (screenshots.length === 0) return null;

  const active = openIdx != null ? screenshots[openIdx] : null;
  const activeSrc = active ? fullSrc(active) : null;

  return (
    <div className="stack">
      <div className="page-header">
        <div>
          <h2 className="m-0 text-base font-semibold">Screenshots</h2>
          <p className="page-subtitle">{screenshots.length} images · click to enlarge</p>
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
          const src = thumbSrc(s);
          return (
            <button
              key={idx}
              type="button"
              className="gallery-thumb"
              onClick={() => setOpenIdx(idx)}
              aria-label={`Open screenshot ${idx + 1}`}
            >
              {src ? (
                <img src={src} alt="" loading="lazy" />
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
                disabled={busy}
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
              <img src={activeSrc} alt="" className="lightbox-image" />
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
              const src = thumbSrc(s);
              return (
                <button
                  key={idx}
                  type="button"
                  className={`lightbox-strip-item ${idx === openIdx ? "is-active" : ""}`}
                  onClick={() => setOpenIdx(idx)}
                >
                  {src && <img src={src} alt="" />}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
