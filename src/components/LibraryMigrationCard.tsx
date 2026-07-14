import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { FolderTree, RefreshCw } from "lucide-react";
import { api } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import type { MigrationArchiveItem, MigrationStatus, Platform } from "@/lib/types";
import { PLATFORMS, platformLabel } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

function MigrationRow({
  item,
  onUpdated,
}: {
  item: MigrationArchiveItem;
  onUpdated: () => void;
}) {
  const { runTask } = useTasks();
  const [platform, setPlatform] = useState<Platform>(item.platform);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setPlatform(item.platform);
  }, [item.platform]);

  const handleSave = async () => {
    if (platform === item.platform && !item.is_legacy_path) return;
    setSaving(true);
    try {
      await runTask(
        `platform-${item.id}`,
        `Updating ${item.filename}`,
        async () =>
          api.assignArchivePlatform(item.game_id, item.id, platform, true),
      );
      onUpdated();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to update platform");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-2 rounded-lg border border-[var(--color-border)] p-3">
      <div className="min-w-0 flex-1">
        <p className="break-all text-sm font-medium">{item.game_title}</p>
        <p className="break-all text-xs text-[var(--color-muted-foreground)]">
          {item.filename}
        </p>
        <div className="mt-1 flex flex-wrap gap-1">
          {item.is_legacy_path && (
            <span className="rounded bg-amber-500/15 px-1.5 py-0.5 text-[10px] text-amber-200">
              flat path
            </span>
          )}
          {item.needs_platform && (
            <span className="rounded bg-blue-500/15 px-1.5 py-0.5 text-[10px] text-blue-200">
              needs platform
            </span>
          )}
        </div>
      </div>
      <select
        value={platform}
        onChange={(e) => setPlatform(e.target.value as Platform)}
        className="rounded-md border border-[var(--color-border)] bg-[var(--color-background)] px-2 py-1 text-sm"
      >
        {PLATFORMS.filter((p) => p !== "unknown").map((p) => (
          <option key={p} value={p}>
            {platformLabel(p)}
          </option>
        ))}
      </select>
      <Button size="sm" onClick={handleSave} disabled={saving}>
        {saving ? "Saving…" : "Apply"}
      </Button>
    </div>
  );
}

export function LibraryMigrationCard() {
  const { runTask } = useTasks();
  const [status, setStatus] = useState<MigrationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setStatus(await api.getMigrationStatus());
    } catch {
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleReorganizeAll = async () => {
    setMessage(null);
    try {
      const result = await runTask(
        "reorganize-archives",
        "Reorganizing matched library archives",
        async (update) => {
          update("Moving matched games into games/{id}/platforms/…");
          return api.reorganizeArchives();
        },
      );
      const parts = [
        `Moved ${result.moved} archive(s).`,
        result.skipped_unknown > 0
          ? `Skipped ${result.skipped_unknown} without a platform.`
          : null,
        result.skipped_already_structured > 0
          ? `Skipped ${result.skipped_already_structured} already organized.`
          : null,
        result.skipped_missing > 0
          ? `Skipped ${result.skipped_missing} missing on disk.`
          : null,
        result.failed > 0 ? `${result.failed} failed.` : null,
      ].filter(Boolean);
      const detail =
        result.errors.length > 0 ? `\n\n${result.errors.join("\n")}` : "";
      setMessage(parts.join(" ") + detail);
      await load();
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Reorganization failed");
    }
  };

  if (loading || !status || status.needs_attention === 0) {
    return null;
  }

  return (
    <Card id="migration">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <FolderTree className="h-4 w-4" />
          Library migration
        </CardTitle>
        <CardDescription>
          Matched games still using flat archive paths from before the platform
          update. Assign a platform and move them into{" "}
          <code className="text-xs">games/&#123;id&#125;/platforms/&#123;platform&#125;/</code>
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <p className="text-sm text-[var(--color-muted-foreground)]">
          {status.needs_attention} matched{" "}
          {status.needs_attention === 1 ? "game needs" : "games need"} attention
          ({status.legacy_paths} flat path, {status.unknown_platforms} unknown
          platform). Unmatched files in your archive folder are fine until you
          match them.
        </p>
        <div className="flex flex-wrap gap-2">
          <Button onClick={handleReorganizeAll}>
            <RefreshCw className="h-4 w-4" />
            Reorganize all assigned
          </Button>
        </div>
        <div className="max-h-96 space-y-2 overflow-y-auto">
          {status.archives.map((item) => (
            <MigrationRow key={item.id} item={item} onUpdated={load} />
          ))}
        </div>
        {message && (
          <p className="break-words text-sm text-[var(--color-muted-foreground)]">
            {message}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

export function MigrationBanner() {
  const [needsAttention, setNeedsAttention] = useState(0);

  useEffect(() => {
    api
      .getMigrationStatus()
      .then((s) => setNeedsAttention(s.needs_attention))
      .catch(() => setNeedsAttention(0));
  }, []);

  if (needsAttention === 0) return null;

  return (
    <div className="mb-6 rounded-lg border border-amber-500/40 bg-amber-500/10 px-4 py-3 text-sm">
      <p>
        <strong>{needsAttention} matched</strong>{" "}
        {needsAttention === 1 ? "game has" : "games have"} archives on legacy
        flat paths.{" "}
        <Link to="/settings#migration" className="underline underline-offset-2">
          Migrate in Settings
        </Link>
      </p>
    </div>
  );
}
