import { useEffect, useState } from "react";

const PREVIEW_INTERVAL_MS = 900;

interface CoverPreviewProps {
  coverUrl: string | null;
  previewUrls?: string[];
  alt: string;
}

export function CoverPreview({
  coverUrl,
  previewUrls = [],
  alt,
}: CoverPreviewProps) {
  const frames =
    previewUrls.length > 0
      ? previewUrls
      : coverUrl
        ? [coverUrl]
        : [];

  const [activeIndex, setActiveIndex] = useState(0);
  const [hovered, setHovered] = useState(false);

  useEffect(() => {
    if (!hovered || frames.length <= 1) return;
    const id = window.setInterval(() => {
      setActiveIndex((i) => (i + 1) % frames.length);
    }, PREVIEW_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [hovered, frames.length]);

  useEffect(() => {
    if (!hovered) setActiveIndex(0);
  }, [hovered]);

  if (frames.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-muted-foreground)]">
        No cover
      </div>
    );
  }

  return (
    <div
      className="relative h-full w-full"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {frames.map((url, i) => (
        <img
          key={`${url}-${i}`}
          src={url}
          alt={i === 0 ? alt : ""}
          className={`absolute inset-0 h-full w-full object-cover transition-opacity duration-500 ease-in-out ${
            i === activeIndex ? "opacity-100" : "opacity-0"
          }`}
        />
      ))}
      {hovered && frames.length > 1 && (
        <div className="absolute bottom-2 left-0 right-0 flex justify-center gap-1">
          {frames.map((_, i) => (
            <span
              key={i}
              className={`h-1.5 rounded-full transition-all duration-300 ${
                i === activeIndex
                  ? "w-4 bg-white"
                  : "w-1.5 bg-white/40"
              }`}
            />
          ))}
        </div>
      )}
    </div>
  );
}
