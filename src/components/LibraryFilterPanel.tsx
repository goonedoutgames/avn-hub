import type { LibrarySort, PlayStatus } from "@/lib/types";
import { playStatusLabel } from "@/components/GameUserNotesCard";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const PLAY_STATUSES: PlayStatus[] = [
  "unplayed",
  "playing",
  "completed",
  "dropped",
];

const SORT_OPTIONS: { value: LibrarySort; label: string }[] = [
  { value: "title", label: "Title (A–Z)" },
  { value: "title_desc", label: "Title (Z–A)" },
  { value: "f95_rating", label: "F95 rating (high → low)" },
  { value: "f95_rating_asc", label: "F95 rating (low → high)" },
  { value: "user_rating", label: "Your rating (high → low)" },
  { value: "user_rating_asc", label: "Your rating (low → high)" },
  { value: "play_status", label: "Play status" },
  { value: "play_status_desc", label: "Play status (reverse)" },
];

export interface LibraryFilterState {
  sort: LibrarySort;
  playStatusFilter: PlayStatus[];
  minF95Rating: string;
  minUserRating: string;
}

interface LibraryFilterPanelProps {
  sort: LibrarySort;
  playStatusFilter: PlayStatus[];
  minF95Rating: string;
  minUserRating: string;
  onSortChange: (sort: LibrarySort) => void;
  onPlayStatusFilterChange: (statuses: PlayStatus[]) => void;
  onMinF95RatingChange: (value: string) => void;
  onMinUserRatingChange: (value: string) => void;
  onClear: () => void;
  disabled?: boolean;
}

export function LibraryFilterPanel({
  sort,
  playStatusFilter,
  minF95Rating,
  minUserRating,
  onSortChange,
  onPlayStatusFilterChange,
  onMinF95RatingChange,
  onMinUserRatingChange,
  onClear,
  disabled,
}: LibraryFilterPanelProps) {
  const togglePlayStatus = (status: PlayStatus) => {
    if (playStatusFilter.includes(status)) {
      onPlayStatusFilterChange(playStatusFilter.filter((s) => s !== status));
    } else {
      onPlayStatusFilterChange([...playStatusFilter, status]);
    }
  };

  const hasMetaFilters =
    playStatusFilter.length > 0 ||
    minF95Rating.trim() !== "" ||
    minUserRating.trim() !== "" ||
    sort !== "title";

  return (
    <div className="space-y-6">
      <section className="space-y-2">
        <h3 className="text-sm font-medium">Sort by</h3>
        <select
          value={sort}
          onChange={(e) => onSortChange(e.target.value as LibrarySort)}
          disabled={disabled}
          className="flex h-9 w-full rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-ring)]"
        >
          {SORT_OPTIONS.map(({ value, label }) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium">Play status</h3>
        <p className="text-xs text-[var(--color-muted-foreground)]">
          Leave all unchecked to show every status. Games default to Not
          started.
        </p>
        <div className="flex flex-wrap gap-2">
          {PLAY_STATUSES.map((status) => {
            const active = playStatusFilter.includes(status);
            return (
              <button
                key={status}
                type="button"
                disabled={disabled}
                onClick={() => togglePlayStatus(status)}
                className={cn(
                  "rounded-full border px-3 py-1 text-xs transition-colors",
                  active
                    ? "border-[var(--color-primary)] bg-[var(--color-primary)]/15 text-[var(--color-foreground)]"
                    : "border-[var(--color-border)] bg-[var(--color-secondary)] text-[var(--color-muted-foreground)] hover:border-[var(--color-primary)]/50",
                )}
              >
                {playStatusLabel(status)}
              </button>
            );
          })}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-medium">Minimum ratings</h3>
        <label className="block space-y-1">
          <span className="text-xs text-[var(--color-muted-foreground)]">
            F95 community rating (0–5)
          </span>
          <input
            type="number"
            min={0}
            max={5}
            step={0.1}
            value={minF95Rating}
            onChange={(e) => onMinF95RatingChange(e.target.value)}
            disabled={disabled}
            placeholder="Any"
            className="flex h-9 w-full rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-ring)]"
          />
        </label>
        <label className="block space-y-1">
          <span className="text-xs text-[var(--color-muted-foreground)]">
            Your rating (1–5)
          </span>
          <input
            type="number"
            min={1}
            max={5}
            step={1}
            value={minUserRating}
            onChange={(e) => onMinUserRatingChange(e.target.value)}
            disabled={disabled}
            placeholder="Any"
            className="flex h-9 w-full rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-ring)]"
          />
        </label>
      </section>

      {hasMetaFilters && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={onClear}
          disabled={disabled}
        >
          Reset sort & filters
        </Button>
      )}
    </div>
  );
}

export function libraryFilterSummary(state: LibraryFilterState): string[] {
  const chips: string[] = [];
  if (state.sort !== "title") {
    const label = SORT_OPTIONS.find((o) => o.value === state.sort)?.label;
    if (label) chips.push(`Sort: ${label}`);
  }
  for (const status of state.playStatusFilter) {
    chips.push(playStatusLabel(status));
  }
  if (state.minF95Rating.trim()) {
    chips.push(`F95 ≥ ${state.minF95Rating}`);
  }
  if (state.minUserRating.trim()) {
    chips.push(`Yours ≥ ${state.minUserRating}`);
  }
  return chips;
}

export function buildLibraryListParams(
  nameSearch: string,
  selectedTags: string[],
  tagMode: import("@/lib/types").TagFilterMode,
  filters: LibraryFilterState,
): import("@/lib/types").LibraryListParams {
  const minF95 = filters.minF95Rating.trim();
  const minUser = filters.minUserRating.trim();
  return {
    search: nameSearch.trim() || undefined,
    tags:
      selectedTags.length > 0 ? selectedTags.join(",") : undefined,
    tagsMode: selectedTags.length > 0 ? tagMode : undefined,
    playStatus:
      filters.playStatusFilter.length > 0
        ? filters.playStatusFilter
        : undefined,
    minF95Rating: minF95 ? Number(minF95) : undefined,
    minUserRating: minUser ? Number(minUser) : undefined,
    sort: filters.sort !== "title" ? filters.sort : undefined,
  };
}
