import { Eye, Star, ThumbsUp } from "lucide-react";
import { HoverMedia } from "@/components/HoverMedia";
import { PlatformBadges } from "@/components/PlatformBadges";
import { TagBadges } from "@/components/TagBadges";
import type { F95SearchResult } from "@/lib/types";
import { useMemo } from "react";

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
  inLibrary?: boolean;
  onAdd: () => void;
  onOpen?: () => void;
};

export function CatalogGameCard({ game, busy, inLibrary, onAdd, onOpen }: Props) {
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

  return (
    <article
      className="card group overflow-hidden transition hover:border-[var(--accent-dim)]"
      onClick={onOpen}
      onKeyDown={(e) => {
        if (!onOpen) return;
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen();
        }
      }}
      role={onOpen ? "button" : undefined}
      tabIndex={onOpen ? 0 : undefined}
    >
      {gallery.length > 0 ? (
        <HoverMedia
          images={gallery}
          referrerPolicy="no-referrer"
          className="aspect-[16/9]"
          imgClassName="transition duration-500 group-hover:scale-[1.02]"
        >
          {inLibrary && (
            <span className="absolute left-2 top-2 z-[1] rounded bg-[var(--accent-dim)] px-1.5 py-0.5 text-[10px] font-semibold uppercase text-white">
              In library
            </span>
          )}
          <div className="pointer-events-none absolute inset-x-0 bottom-0 z-[1] flex items-end justify-between gap-2 bg-gradient-to-t from-black/75 via-black/25 to-transparent p-2 pt-8">
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
        </HoverMedia>
      ) : (
        <div className="relative flex aspect-[16/9] items-center justify-center bg-[var(--bg-soft)] text-sm text-[var(--muted)]">
          {inLibrary && (
            <span className="absolute left-2 top-2 rounded bg-[var(--accent-dim)] px-1.5 py-0.5 text-[10px] font-semibold uppercase text-white">
              In library
            </span>
          )}
          No preview
        </div>
      )}

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
        <PlatformBadges platforms={game.platforms} />
        <TagBadges tags={game.tags} limit={4} />
        <div className="flex flex-col gap-2 pt-1 sm:flex-row sm:flex-wrap">
          <button
            type="button"
            className="btn flex-1 text-xs"
            disabled={busy}
            onClick={(e) => {
              e.stopPropagation();
              onOpen?.();
            }}
          >
            Details
          </button>
          <button
            type="button"
            className={`btn flex-1 text-xs ${inLibrary ? "" : "btn-primary"}`}
            disabled={busy || inLibrary}
            aria-disabled={inLibrary || busy}
            onClick={(e) => {
              e.stopPropagation();
              if (!inLibrary) onAdd();
            }}
          >
            {inLibrary ? "Added" : busy ? "Adding…" : "Add"}
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
