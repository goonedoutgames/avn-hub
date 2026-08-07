import { useEffect, useState, type ReactNode } from "react";

type Props = {
  images: string[];
  /** Interval between slides while hovering (ms) */
  intervalMs?: number;
  className?: string;
  imgClassName?: string;
  referrerPolicy?: React.ImgHTMLAttributes<HTMLImageElement>["referrerPolicy"];
  children?: ReactNode;
};

/**
 * Cover that, on hover, slowly crossfades through the provided images.
 */
export function HoverMedia({
  images,
  intervalMs = 1800,
  className = "",
  imgClassName = "",
  referrerPolicy,
  children,
}: Props) {
  const gallery = images.filter(Boolean);
  const [hover, setHover] = useState(false);
  const [idx, setIdx] = useState(0);
  const nextSrc =
    hover && gallery.length > 1 ? gallery[(idx + 1) % gallery.length] : undefined;

  useEffect(() => {
    if (!hover || gallery.length <= 1) return;
    const id = window.setInterval(() => {
      setIdx((i) => (i + 1) % gallery.length);
    }, intervalMs);
    return () => window.clearInterval(id);
  }, [hover, gallery.length, intervalMs]);

  useEffect(() => {
    if (!hover) setIdx(0);
  }, [hover]);

  // Prefetch neighbors while hovering so fades aren't blank
  useEffect(() => {
    if (!nextSrc) return;
    const img = new Image();
    img.src = nextSrc;
  }, [nextSrc]);

  return (
    <div
      className={`hover-media ${className}`.trim()}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
    >
      {gallery.length === 0 ? (
        <div className="hover-media-empty">{children}</div>
      ) : (
        <>
          {gallery.map((src, i) => (
            <img
              key={`${src}-${i}`}
              src={src}
              alt=""
              referrerPolicy={referrerPolicy}
              loading={i === 0 ? "lazy" : "eager"}
              className={`hover-media-img ${i === idx ? "is-active" : ""} ${imgClassName}`.trim()}
            />
          ))}
          {children}
          {hover && gallery.length > 1 && (
            <div className="hover-media-dots" aria-hidden>
              {gallery.slice(0, 8).map((_, i) => (
                <span
                  key={i}
                  className={`hover-media-dot ${
                    i === idx % Math.min(gallery.length, 8) ? "is-active" : ""
                  }`}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
