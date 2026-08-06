import { useEffect, useMemo, useState } from "react";
import { Eye, Star, ThumbsUp } from "lucide-react";
import type { F95SearchResult } from "@/lib/types";

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

function formatCount(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 10_000 ? 0 : 1)}K`;
  return String(n);
}

type Props = {
  game: F95SearchResult;
  busy?: boolean;
  onAdd: () => void;
};

export function CatalogGameCard({ game, busy, onAdd }: Props) {
  const gallery = useMemo(() => {
    const urls = [game.cover, ...game.screenshots].filter(Boolean);
    const seen = new Set<string>();
    return urls.filter((u) => {
      const key = u.toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [game.cover, game.screenshots]);

  const [hover, setHover] = useState(false);
  const [idx, setIdx] = useState(0);

  useEffect(() => {
    if (!hover || gallery.length <= 1) return;
    const id = window.setInterval(() => {
      setIdx((i) => (i + 1) % gallery.length);
    }, 900);
    return () => window.clearInterval(id);
  }, [hover, gallery.length]);

  useEffect(() => {
    if (!hover) setIdx(0);
  }, [hover]);

  const image = gallery[idx] || game.cover;
  const tags = game.tags.filter((t) => !/^\d+$/.test(t)).slice(0, 4);

  return (
    <article
      className="card group overflow-hidden transition hover:border-[var(--accent-dim)]"
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
    >
      <div className="relative aspect-[16/9] overflow-hidden bg-[var(--bg-soft)]">
        {image ? (
          <img
            src={image}
            alt=""
            referrerPolicy="no-referrer"
            className="h-full w-full object-cover transition duration-300 group-hover:scale-[1.02]"
            loading="lazy"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-[var(--muted)]">
            No preview
          </div>
        )}

        <div className="pointer-events-none absolute inset-x-0 bottom-0 flex items-end justify-between gap-2 bg-gradient-to-t from-black/75 via-black/25 to-transparent p-2 pt-8">
          <div className="flex flex-wrap gap-1">
            {game.prefixes?.slice(0, 3).map((p) => (
              <span
                key={p}
                className={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-white ${prefixClass(p)}`}
              >
                {p}
              </span>
            ))}
          </div>
          {game.version && (
            <span className="rounded bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white">
              {game.version.startsWith("v") || game.version.startsWith("V")
                ? game.version
                : `v${game.version}`}
            </span>
          )}
        </div>

        {hover && gallery.length > 1 && (
          <div className="absolute bottom-2 left-1/2 flex -translate-x-1/2 gap-1">
            {gallery.slice(0, 8).map((_, i) => (
              <span
                key={i}
                className={`h-1.5 w-1.5 rounded-full ${
                  i === idx % Math.min(gallery.length, 8) ? "bg-white" : "bg-white/40"
                }`}
              />
            ))}
          </div>
        )}
      </div>

      <div className="space-y-2 p-3">
        <h3 className="line-clamp-2 text-sm font-semibold leading-snug">{game.title}</h3>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-[var(--muted)]">
          {game.date && <span>{game.date}</span>}
          {game.likes != null && (
            <span className="inline-flex items-center gap-1">
              <ThumbsUp className="h-3 w-3" /> {formatCount(game.likes)}
            </span>
          )}
          {game.views != null && (
            <span className="inline-flex items-center gap-1">
              <Eye className="h-3 w-3" /> {formatCount(game.views)}
            </span>
          )}
          {game.rating > 0 && (
            <span className="inline-flex items-center gap-1">
              <Star className="h-3 w-3" /> {game.rating.toFixed(1)}
            </span>
          )}
          <span className="truncate">{game.creator}</span>
        </div>
        {tags.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {tags.map((t) => (
              <span
                key={t}
                className="rounded border border-[var(--border)] px-1.5 py-0.5 text-[10px] text-[var(--muted)]"
              >
                {t}
              </span>
            ))}
          </div>
        )}
        <div className="flex flex-wrap gap-2 pt-1">
          <button
            type="button"
            className="btn btn-primary flex-1 text-xs"
            disabled={busy}
            onClick={onAdd}
          >
            {busy ? "Adding…" : "Add"}
          </button>
          <a
            className="btn text-xs"
            href={game.url}
            target="_blank"
            rel="noreferrer"
            onClick={(e) => e.stopPropagation()}
          >
            F95
          </a>
        </div>
      </div>
    </article>
  );
}
