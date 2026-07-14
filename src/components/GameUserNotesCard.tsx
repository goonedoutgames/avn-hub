import { useEffect, useState } from "react";
import { Save, Star } from "lucide-react";
import { api } from "@/lib/api";
import type { Game, PlayStatus } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const PLAY_STATUSES: { value: PlayStatus; label: string }[] = [
  { value: "unplayed", label: "Not started" },
  { value: "playing", label: "Playing" },
  { value: "completed", label: "Completed" },
  { value: "dropped", label: "Dropped" },
];

interface GameUserNotesCardProps {
  game: Game;
  onUpdated: (game: Game) => void;
}

export function GameUserNotesCard({ game, onUpdated }: GameUserNotesCardProps) {
  const [playStatus, setPlayStatus] = useState<PlayStatus>(
    game.play_status ?? "unplayed",
  );
  const [userRating, setUserRating] = useState<number | null>(
    game.user_rating ?? null,
  );
  const [notes, setNotes] = useState(game.user_notes ?? "");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    setPlayStatus(game.play_status ?? "unplayed");
    setUserRating(game.user_rating ?? null);
    setNotes(game.user_notes ?? "");
  }, [game.id, game.play_status, game.user_rating, game.user_notes]);

  const handleSave = async () => {
    setSaving(true);
    setMessage(null);
    try {
      const result = await api.updateGameUserData(game.id, {
        play_status: playStatus,
        user_rating: userRating,
        user_notes: notes.trim() || null,
      });
      onUpdated(result.game);
      setMessage("Saved");
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Your notes</CardTitle>
        <CardDescription>
          Personal play status, rating, and review notes (only stored locally)
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs font-medium text-[var(--color-muted-foreground)]">
            Play status
          </label>
          <select
            value={playStatus}
            onChange={(e) => setPlayStatus(e.target.value as PlayStatus)}
            className="flex h-9 w-full rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-ring)]"
          >
            {PLAY_STATUSES.map(({ value, label }) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="mb-1.5 block text-xs font-medium text-[var(--color-muted-foreground)]">
            Your rating
          </label>
          <div className="flex items-center gap-1">
            {[1, 2, 3, 4, 5].map((star) => (
              <button
                key={star}
                type="button"
                aria-label={`Rate ${star} stars`}
                onClick={() =>
                  setUserRating(userRating === star ? null : star)
                }
                className="rounded p-0.5 transition-colors hover:scale-110"
              >
                <Star
                  className={`h-6 w-6 ${
                    userRating != null && star <= userRating
                      ? "fill-yellow-400 text-yellow-400"
                      : "text-[var(--color-muted-foreground)]"
                  }`}
                />
              </button>
            ))}
            {userRating != null && (
              <button
                type="button"
                className="ml-2 text-xs text-[var(--color-muted-foreground)] underline"
                onClick={() => setUserRating(null)}
              >
                Clear
              </button>
            )}
          </div>
        </div>

        <div>
          <label className="mb-1.5 block text-xs font-medium text-[var(--color-muted-foreground)]">
            Notes
          </label>
          <textarea
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={4}
            placeholder="Thoughts, route notes, things to try next time…"
            className="flex w-full rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-ring)]"
          />
        </div>

        <div className="flex items-center gap-3">
          <Button size="sm" onClick={handleSave} disabled={saving}>
            <Save className="h-3 w-3" />
            {saving ? "Saving…" : "Save notes"}
          </Button>
          {message && (
            <span className="text-xs text-[var(--color-muted-foreground)]">
              {message}
            </span>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export function playStatusLabel(status: PlayStatus | null | undefined): string {
  return (
    PLAY_STATUSES.find((s) => s.value === status)?.label ?? "Not started"
  );
}

export function PlayStatusBadge({
  status,
}: {
  status: PlayStatus | null | undefined;
}) {
  if (!status || status === "unplayed") return null;
  const colors: Record<PlayStatus, string> = {
    unplayed: "",
    playing: "bg-blue-500/20 text-blue-300",
    completed: "bg-green-500/20 text-green-300",
    dropped: "bg-orange-500/20 text-orange-300",
  };
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${colors[status]}`}
    >
      {playStatusLabel(status)}
    </span>
  );
}
