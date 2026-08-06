import { Star } from "lucide-react";

export const PLAY_STATUSES = [
  { value: "unplayed", label: "Unplayed" },
  { value: "playing", label: "Playing" },
  { value: "completed", label: "Completed" },
  { value: "dropped", label: "Dropped" },
] as const;

export type PlayStatusValue = (typeof PLAY_STATUSES)[number]["value"];

export function playStatusLabel(status: string | null | undefined): string {
  const found = PLAY_STATUSES.find((s) => s.value === status);
  return found?.label ?? status ?? "Unplayed";
}

type BadgeProps = {
  status: string | null | undefined;
  size?: "sm" | "md";
  className?: string;
};

export function PlayStatusBadge({ status, size = "sm", className = "" }: BadgeProps) {
  const key = (status || "unplayed").toLowerCase();
  return (
    <span className={`status-badge status-${key} status-${size} ${className}`.trim()}>
      {playStatusLabel(key)}
    </span>
  );
}

type StarProps = {
  value: number | null | undefined;
  /** Max stars (F95 often uses 5). */
  max?: number;
  size?: "sm" | "md" | "lg";
  /** Interactive picker */
  onChange?: (value: number | null) => void;
  disabled?: boolean;
  /** Show numeric label next to stars */
  showValue?: boolean;
  /** Optional prefix label e.g. "F95" / "Yours" */
  label?: string;
  className?: string;
};

function clampRating(n: number, max: number): number {
  const stepped = Math.round(n * 2) / 2;
  return Math.min(max, Math.max(0.5, stepped));
}

export function StarRating({
  value,
  max = 5,
  size = "md",
  onChange,
  disabled,
  showValue = false,
  label,
  className = "",
}: StarProps) {
  const rating = value != null && value > 0 ? value : 0;
  const interactive = Boolean(onChange) && !disabled;

  const setFromStar = (starIndex: number, half: boolean) => {
    if (!onChange) return;
    const next = clampRating(starIndex + (half ? 0.5 : 1), max);
    // Clicking the current full value again clears
    if (rating === next) {
      onChange(null);
    } else {
      onChange(next);
    }
  };

  return (
    <div
      className={`star-rating star-${size} ${interactive ? "star-interactive" : ""} ${className}`.trim()}
      role={interactive ? "slider" : "img"}
      aria-label={label ? `${label} rating` : "Rating"}
      aria-valuemin={interactive ? 0 : undefined}
      aria-valuemax={interactive ? max : undefined}
      aria-valuenow={interactive ? rating || 0 : undefined}
    >
      {label && <span className="star-rating-label">{label}</span>}
      <span className="star-rating-stars">
        {Array.from({ length: max }, (_, i) => {
          const fill = Math.min(1, Math.max(0, rating - i));
          return (
            <span key={i} className="star-slot">
              {interactive && (
                <>
                  <button
                    type="button"
                    className="star-hit star-hit-left"
                    aria-label={`${i + 0.5} stars`}
                    disabled={disabled}
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setFromStar(i, true);
                    }}
                  />
                  <button
                    type="button"
                    className="star-hit star-hit-right"
                    aria-label={`${i + 1} stars`}
                    disabled={disabled}
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setFromStar(i, false);
                    }}
                  />
                </>
              )}
              <Star className="star-icon star-empty" aria-hidden />
              <span className="star-fill" style={{ width: `${fill * 100}%` }} aria-hidden>
                <Star className="star-icon star-full" />
              </span>
            </span>
          );
        })}
      </span>
      {showValue && (
        <span className="star-rating-value">
          {rating > 0 ? rating.toFixed(1) : "—"}
        </span>
      )}
    </div>
  );
}
