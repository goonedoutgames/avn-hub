import { useCallback, useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, Expand, ImageIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Lightbox } from "@/components/Lightbox";
import { screenshotDisplayUrl } from "@/lib/image-url";
import type { ScreenshotItem } from "@/lib/types";

interface ScreenshotGalleryProps {
  screenshots: ScreenshotItem[];
  coverUrl: string | null;
  onSetCover: (index: number) => Promise<void>;
}

export function ScreenshotGallery({
  screenshots,
  coverUrl,
  onSetCover,
}: ScreenshotGalleryProps) {
  const [index, setIndex] = useState(0);
  const [settingCover, setSettingCover] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);

  useEffect(() => {
    setIndex((i) => Math.min(i, Math.max(0, screenshots.length - 1)));
  }, [screenshots.length]);

  const count = screenshots.length;

  const goPrev = useCallback(
    () => setIndex((i) => (i - 1 + Math.max(count, 1)) % Math.max(count, 1)),
    [count],
  );
  const goNext = useCallback(
    () => setIndex((i) => (i + 1) % Math.max(count, 1)),
    [count],
  );

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (count <= 1) return;
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        (e.target as HTMLElement | null)?.isContentEditable
      ) {
        return;
      }

      if (e.key === "ArrowLeft") {
        e.preventDefault();
        goPrev();
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        goNext();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [count, goPrev, goNext]);

  if (screenshots.length === 0) return null;

  const current = screenshots[index];
  const displayUrl = screenshotDisplayUrl(current);
  const isCurrentCover =
    coverUrl != null &&
    (coverUrl === current.cached_url || coverUrl === current.full_url);

  const handleSetCover = async () => {
    setSettingCover(true);
    try {
      await onSetCover(index);
    } finally {
      setSettingCover(false);
    }
  };

  return (
    <>
      <div className="space-y-3">
        <div className="group relative aspect-video overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-muted)]">
          <button
            type="button"
            onClick={() => setLightboxOpen(true)}
            className="h-full w-full cursor-zoom-in"
            aria-label={`View full resolution screenshot ${index + 1}`}
          >
            <img
              src={displayUrl}
              alt={`Screenshot ${index + 1} of ${count}`}
              className="h-full w-full object-contain"
              loading="eager"
              decoding="async"
            />
          </button>

          {count > 1 && (
            <>
              <button
                type="button"
                onClick={goPrev}
                className="absolute left-2 top-1/2 -translate-y-1/2 rounded-full bg-black/50 p-2 text-white opacity-0 transition-opacity hover:bg-black/70 group-hover:opacity-100"
                aria-label="Previous screenshot"
              >
                <ChevronLeft className="h-5 w-5" />
              </button>
              <button
                type="button"
                onClick={goNext}
                className="absolute right-2 top-1/2 -translate-y-1/2 rounded-full bg-black/50 p-2 text-white opacity-0 transition-opacity hover:bg-black/70 group-hover:opacity-100"
                aria-label="Next screenshot"
              >
                <ChevronRight className="h-5 w-5" />
              </button>
              <div className="pointer-events-none absolute bottom-2 left-1/2 -translate-x-1/2 rounded-full bg-black/50 px-2.5 py-0.5 text-xs text-white">
                {index + 1} / {count}
              </div>
            </>
          )}

          <div className="absolute right-2 top-2 flex items-center gap-2">
            <span className="pointer-events-none rounded-full bg-black/50 p-2 text-white opacity-0 transition-opacity group-hover:opacity-100">
              <Expand className="h-4 w-4" />
            </span>
            <Button
              size="sm"
              variant={isCurrentCover ? "secondary" : "default"}
              onClick={handleSetCover}
              disabled={settingCover || isCurrentCover}
            >
              <ImageIcon className="h-3.5 w-3.5" />
              {isCurrentCover
                ? "Current cover"
                : settingCover
                  ? "Setting…"
                  : "Set as cover"}
            </Button>
          </div>
        </div>

        <div className="flex gap-2 overflow-x-auto pb-1">
          {screenshots.map((shot, i) => {
            const stripUrl = screenshotDisplayUrl(shot);
            const isCover =
              coverUrl != null &&
              (coverUrl === shot.cached_url || coverUrl === shot.full_url);
            return (
              <button
                key={`${shot.full_url}-${i}`}
                type="button"
                onClick={() => setIndex(i)}
                onDoubleClick={() => {
                  setIndex(i);
                  setLightboxOpen(true);
                }}
                title="Click to select, double-click for full size"
                className={`relative shrink-0 overflow-hidden rounded-md border-2 transition-colors ${
                  i === index
                    ? "border-[var(--color-primary)]"
                    : "border-transparent opacity-70 hover:opacity-100"
                }`}
              >
                <img
                  src={stripUrl}
                  alt=""
                  className="h-16 w-28 object-cover"
                  loading="lazy"
                />
                {isCover && (
                  <span className="absolute bottom-0 left-0 right-0 bg-black/60 py-0.5 text-center text-[9px] text-white">
                    Cover
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      <Lightbox
        src={displayUrl}
        alt={`Screenshot ${index + 1} of ${count}`}
        open={lightboxOpen}
        onClose={() => setLightboxOpen(false)}
        onPrev={count > 1 ? goPrev : undefined}
        onNext={count > 1 ? goNext : undefined}
        position={count > 1 ? `${index + 1} / ${count}` : undefined}
      />
    </>
  );
}
