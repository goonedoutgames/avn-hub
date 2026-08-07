import { useState } from "react";

export function humanTags(tags: string[] | null | undefined): string[] {
  return (tags ?? []).map((t) => t.trim()).filter((t) => t.length > 0 && !/^\d+$/.test(t));
}

type Props = {
  tags: string[];
  /** How many to show before collapsing */
  limit?: number;
  size?: "sm" | "md";
  className?: string;
  onTagClick?: (tag: string) => void;
};

export function TagBadges({
  tags,
  limit = 4,
  size = "sm",
  className = "",
  onTagClick,
}: Props) {
  const clean = humanTags(tags);
  const [expanded, setExpanded] = useState(false);

  if (clean.length === 0) return null;

  const visible = expanded ? clean : clean.slice(0, limit);
  const hidden = Math.max(0, clean.length - limit);

  return (
    <div className={`tag-badge-row ${className}`.trim()}>
      {visible.map((t) =>
        onTagClick ? (
          <button
            key={t}
            type="button"
            className={`tag-badge tag-${size} tag-clickable`}
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onTagClick(t);
            }}
          >
            {t}
          </button>
        ) : (
          <span key={t} className={`tag-badge tag-${size}`}>
            {t}
          </span>
        ),
      )}
      {!expanded && hidden > 0 && (
        <button
          type="button"
          className={`tag-badge tag-${size} tag-more`}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setExpanded(true);
          }}
        >
          +{hidden} more
        </button>
      )}
      {expanded && clean.length > limit && (
        <button
          type="button"
          className={`tag-badge tag-${size} tag-more`}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setExpanded(false);
          }}
        >
          Show less
        </button>
      )}
    </div>
  );
}
