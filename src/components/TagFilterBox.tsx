import { useMemo, useState } from "react";
import { Filter, X } from "lucide-react";
import type { LibraryTag, TagFilterMode } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface TagFilterPanelProps {
  availableTags: LibraryTag[];
  selectedTags: string[];
  mode: TagFilterMode;
  onSelectedTagsChange: (tags: string[]) => void;
  onModeChange: (mode: TagFilterMode) => void;
  onClear: () => void;
  disabled?: boolean;
}

const SUGGESTION_LIMIT = 48;

export function TagFilterPanel({
  availableTags,
  selectedTags,
  mode,
  onSelectedTagsChange,
  onModeChange,
  onClear,
  disabled,
}: TagFilterPanelProps) {
  const [query, setQuery] = useState("");

  const selectedLower = useMemo(
    () => new Set(selectedTags.map((t) => t.toLowerCase())),
    [selectedTags],
  );

  const suggestions = useMemo(() => {
    const q = query.trim().toLowerCase();
    return availableTags
      .filter(({ tag }) => !selectedLower.has(tag.toLowerCase()))
      .filter(({ tag }) => !q || tag.toLowerCase().includes(q))
      .slice(0, SUGGESTION_LIMIT);
  }, [availableTags, query, selectedLower]);

  const addTag = (tag: string) => {
    const key = tag.toLowerCase();
    if (selectedLower.has(key)) return;
    onSelectedTagsChange([...selectedTags, tag]);
    setQuery("");
  };

  const removeTag = (tag: string) => {
    const key = tag.toLowerCase();
    onSelectedTagsChange(
      selectedTags.filter((t) => t.toLowerCase() !== key),
    );
  };

  const hasActiveFilters = selectedTags.length > 0;

  return (
    <div className="space-y-4">
      {hasActiveFilters && (
        <div className="flex justify-end">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onClear}
            disabled={disabled}
          >
            <X className="h-4 w-4" />
            Clear filters
          </Button>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        <div className="inline-flex rounded-md border border-[var(--color-border)] p-0.5">
          <button
            type="button"
            disabled={disabled}
            onClick={() => onModeChange("and")}
            className={`rounded px-2.5 py-1 text-xs font-medium transition-colors ${
              mode === "and"
                ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
                : "text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]"
            }`}
          >
            AND
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={() => onModeChange("or")}
            className={`rounded px-2.5 py-1 text-xs font-medium transition-colors ${
              mode === "or"
                ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
                : "text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]"
            }`}
          >
            OR
          </button>
        </div>
        <span className="text-xs text-[var(--color-muted-foreground)]">
          {mode === "and"
            ? "Must have every tag"
            : "Needs at least one tag"}
        </span>
      </div>

      {selectedTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedTags.map((tag) => (
            <button
              key={tag}
              type="button"
              disabled={disabled}
              onClick={() => removeTag(tag)}
              className="inline-flex items-center gap-1 rounded-full bg-[var(--color-primary)] px-2.5 py-1 text-xs text-[var(--color-primary-foreground)] transition-opacity hover:opacity-90"
            >
              {tag}
              <X className="h-3 w-3" />
            </button>
          ))}
        </div>
      )}

      <Input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={
          availableTags.length > 0
            ? "Search tags from your library…"
            : "Match games to see available tags"
        }
        disabled={disabled || availableTags.length === 0}
        onKeyDown={(e) => {
          if (e.key === "Enter" && suggestions[0]) {
            e.preventDefault();
            addTag(suggestions[0].tag);
          }
        }}
      />

      {availableTags.length > 0 && (
        <div className="space-y-2">
          <p className="flex items-center gap-2 text-xs font-medium text-[var(--color-muted-foreground)]">
            <Filter className="h-3.5 w-3.5" />
            {query.trim() ? "Matching tags" : "Popular tags"}
          </p>
          <div className="flex flex-wrap gap-1.5">
            {suggestions.length > 0 ? (
              suggestions.map(({ tag, count }) => (
                <button
                  key={tag}
                  type="button"
                  disabled={disabled}
                  onClick={() => addTag(tag)}
                  className="rounded-full border border-[var(--color-border)] bg-[var(--color-secondary)] px-2.5 py-1 text-xs transition-colors hover:border-[var(--color-primary)] hover:bg-[var(--color-accent)]"
                >
                  {tag}
                  <span className="ml-1 text-[var(--color-muted-foreground)]">
                    {count}
                  </span>
                </button>
              ))
            ) : (
              <p className="text-xs text-[var(--color-muted-foreground)]">
                No tags match your search.
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
