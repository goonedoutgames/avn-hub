import { useEffect, useRef, useState, type ReactNode } from "react";
import { Download, Star, Trash2, Upload } from "lucide-react";
import {
  ArchiveUpload,
  type ArchiveUploadHandle,
} from "@/components/ArchiveUpload";
import { MobileActionMenu, type ActionMenuItem } from "@/components/MobileActionMenu";
import { api } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import type {
  GameAttachments,
  GamePlatformArchive,
  Platform,
} from "@/lib/types";
import { PLATFORMS, platformLabel } from "@/lib/types";
import { cn, formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

interface GameFilesCardProps {
  gameId: number;
  attachments: GameAttachments;
  onUpdated: () => void;
}

function PlatformSelect({
  value,
  onChange,
  className,
}: {
  value: Platform;
  onChange: (p: Platform) => void;
  className?: string;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as Platform)}
      className={cn(
        "h-9 min-w-0 rounded-md border border-[var(--color-border)] bg-[var(--color-background)] px-2 text-sm",
        className,
      )}
    >
      {PLATFORMS.filter((p) => p !== "unknown").map((p) => (
        <option key={p} value={p}>
          {platformLabel(p)}
        </option>
      ))}
      <option value="unknown">{platformLabel("unknown")}</option>
    </select>
  );
}

function ArchiveRow({
  gameId,
  archive,
  onUpdated,
}: {
  gameId: number;
  archive: GamePlatformArchive;
  onUpdated: () => void;
}) {
  const { runTask } = useTasks();
  const replaceRef = useRef<ArchiveUploadHandle>(null);
  const [platform, setPlatform] = useState<Platform>(archive.platform);
  const needsReorganize = !archive.path.includes(
    `/games/${archive.game_id}/platforms/`,
  );
  const platformChanged = platform !== archive.platform;
  const applyLabel = needsReorganize ? "Apply & move" : "Apply";
  const applyDisabled = !platformChanged && !needsReorganize;

  useEffect(() => {
    setPlatform(archive.platform);
  }, [archive.platform]);

  const handlePlatformSave = async () => {
    if (applyDisabled) return;
    try {
      await runTask(`platform-${archive.id}`, "Updating platform", async () =>
        api.assignArchivePlatform(gameId, archive.id, platform, true),
      );
      onUpdated();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to update platform");
    }
  };

  const handleDownload = async () => {
    try {
      await runTask(
        `dl-archive-${archive.id}`,
        `Downloading ${archive.filename}`,
        async () =>
          api.downloadPlatformArchive(gameId, archive.id, archive.filename),
      );
    } catch (e) {
      alert(e instanceof Error ? e.message : "Download failed");
    }
  };

  const handleSetDefault = async () => {
    try {
      await runTask(`default-${archive.id}`, "Setting default platform", async () =>
        api.setDefaultPlatformArchive(gameId, archive.id),
      );
      onUpdated();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to set default");
    }
  };

  const handleDelete = async () => {
    if (!confirm(`Delete “${archive.filename}”?`)) return;
    try {
      await runTask(`del-archive-${archive.id}`, "Deleting archive", async () =>
        api.deletePlatformArchive(gameId, archive.id),
      );
      onUpdated();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Delete failed");
    }
  };

  const menuItems: ActionMenuItem[] = [
    {
      key: "default",
      label: "Set as default",
      icon: <Star className="h-4 w-4" />,
      onClick: handleSetDefault,
      hidden: archive.is_default,
    },
    {
      key: "replace",
      label: "Replace archive",
      icon: <Upload className="h-4 w-4" />,
      onClick: () => replaceRef.current?.open(),
    },
    {
      key: "delete",
      label: "Delete",
      icon: <Trash2 className="h-4 w-4" />,
      onClick: handleDelete,
      variant: "destructive",
    },
  ];

  return (
    <div className="space-y-3 rounded-lg border border-[var(--color-border)] p-3">
      <div className="min-w-0">
        <p
          className="text-sm font-medium leading-snug break-words"
          title={archive.filename}
        >
          {archive.filename}
        </p>
        <div className="mt-1.5 flex flex-wrap items-center gap-1.5 text-xs text-[var(--color-muted-foreground)]">
          <span className="rounded-md bg-[var(--color-muted)] px-1.5 py-0.5 font-medium text-[var(--color-foreground)]">
            {platformLabel(archive.platform)}
          </span>
          <span>{formatBytes(archive.size)}</span>
          {archive.is_default && (
            <span className="rounded-md bg-[var(--color-muted)] px-1.5 py-0.5 font-medium text-[var(--color-foreground)]">
              Default
            </span>
          )}
        </div>
      </div>

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <PlatformSelect
          value={platform}
          onChange={setPlatform}
          className="w-full sm:flex-1"
        />
        <Button
          size="sm"
          variant="secondary"
          className="w-full sm:w-auto sm:shrink-0"
          onClick={handlePlatformSave}
          disabled={applyDisabled}
        >
          {applyLabel}
        </Button>
      </div>

      <div className="flex gap-2 md:hidden">
        <Button size="sm" className="min-w-0 flex-1" onClick={handleDownload}>
          <Download className="h-4 w-4 shrink-0" />
          Download
        </Button>
        <MobileActionMenu label="Archive" items={menuItems} className="shrink-0" />
      </div>

      <div className="hidden flex-wrap gap-2 md:flex">
        {!archive.is_default && (
          <Button size="sm" variant="outline" onClick={handleSetDefault}>
            <Star className="h-4 w-4" />
            Default
          </Button>
        )}
        <Button size="sm" variant="outline" onClick={handleDownload}>
          <Download className="h-4 w-4" />
          Download
        </Button>
        <ArchiveUpload
          ref={replaceRef}
          gameId={gameId}
          replaceArchiveId={archive.id}
          platform={archive.platform}
          variant="outline"
          size="sm"
          label="Replace"
          onComplete={onUpdated}
        />
        <Button size="sm" variant="outline" onClick={handleDelete}>
          <Trash2 className="h-4 w-4" />
          Delete
        </Button>
      </div>
    </div>
  );
}

function AttachmentRow({
  filename,
  meta,
  onDownload,
  onDelete,
}: {
  filename: string;
  meta: string;
  onDownload: () => void;
  onDelete: () => void;
}) {
  const menuItems: ActionMenuItem[] = [
    {
      key: "download",
      label: "Download",
      icon: <Download className="h-4 w-4" />,
      onClick: onDownload,
    },
    {
      key: "delete",
      label: "Delete",
      icon: <Trash2 className="h-4 w-4" />,
      onClick: onDelete,
      variant: "destructive",
    },
  ];

  return (
    <div className="space-y-3 rounded-lg border border-[var(--color-border)] p-3 md:space-y-0 md:flex md:items-center md:gap-3">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium leading-snug break-words" title={filename}>
          {filename}
        </p>
        <p className="mt-0.5 text-xs text-[var(--color-muted-foreground)]">{meta}</p>
      </div>

      <div className="flex gap-2 md:hidden">
        <Button size="sm" className="min-w-0 flex-1" onClick={onDownload}>
          <Download className="h-4 w-4 shrink-0" />
          Download
        </Button>
        <MobileActionMenu label="Actions" items={menuItems} className="shrink-0" />
      </div>

      <div className="hidden flex-wrap gap-2 md:flex">
        <Button size="sm" variant="outline" onClick={onDownload}>
          <Download className="h-4 w-4" />
          Download
        </Button>
        <Button size="sm" variant="outline" onClick={onDelete}>
          <Trash2 className="h-4 w-4" />
          Delete
        </Button>
      </div>
    </div>
  );
}

function SectionHeader({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
      <h3 className="text-sm font-medium">{title}</h3>
      <div className="flex w-full flex-col gap-2 sm:w-auto sm:flex-row sm:items-center">
        {children}
      </div>
    </div>
  );
}

export function GameFilesCard({ gameId, attachments, onUpdated }: GameFilesCardProps) {
  const { runTask } = useTasks();
  const [uploadPlatform, setUploadPlatform] = useState<Platform>("windows");

  const handleDownloadSave = async (saveId: number, filename: string) => {
    try {
      await runTask(`dl-save-${saveId}`, `Downloading ${filename}`, async () =>
        api.downloadGameSave(gameId, saveId, filename),
      );
    } catch (e) {
      alert(e instanceof Error ? e.message : "Download failed");
    }
  };

  const handleDeleteSave = async (saveId: number, filename: string) => {
    if (!confirm(`Delete save “${filename}”?`)) return;
    try {
      await runTask(`del-save-${saveId}`, "Deleting save", async () =>
        api.deleteGameSave(gameId, saveId),
      );
      onUpdated();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Delete failed");
    }
  };

  const handleDownloadPatch = async (patchId: number, filename: string) => {
    try {
      await runTask(`dl-patch-${patchId}`, `Downloading ${filename}`, async () =>
        api.downloadGamePatch(gameId, patchId, filename),
      );
    } catch (e) {
      alert(e instanceof Error ? e.message : "Download failed");
    }
  };

  const handleDeletePatch = async (patchId: number, filename: string) => {
    if (!confirm(`Delete patch “${filename}”?`)) return;
    try {
      await runTask(`del-patch-${patchId}`, "Deleting patch", async () =>
        api.deleteGamePatch(gameId, patchId),
      );
      onUpdated();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Delete failed");
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Game files</CardTitle>
        <CardDescription>
          Platform archives, save backups, and community patches
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <section className="space-y-3">
          <SectionHeader title="Platform archives">
            <PlatformSelect
              value={uploadPlatform}
              onChange={setUploadPlatform}
              className="w-full sm:w-auto"
            />
            <ArchiveUpload
              gameId={gameId}
              platform={uploadPlatform}
              variant="outline"
              size="sm"
              label="Add archive"
              className="w-full sm:w-auto"
              onComplete={onUpdated}
            />
          </SectionHeader>
          {attachments.platform_archives.length === 0 ? (
            <p className="text-sm text-[var(--color-muted-foreground)]">
              No platform archives yet.
            </p>
          ) : (
            attachments.platform_archives.map((archive) => (
              <ArchiveRow
                key={archive.id}
                gameId={gameId}
                archive={archive}
                onUpdated={onUpdated}
              />
            ))
          )}
        </section>

        <section className="space-y-3">
          <SectionHeader title="Save backups">
            <ArchiveUpload
              gameId={gameId}
              kind="save"
              variant="outline"
              size="sm"
              label="Upload save"
              className="w-full sm:w-auto"
              onComplete={onUpdated}
            />
          </SectionHeader>
          {attachments.saves.length === 0 ? (
            <p className="text-sm text-[var(--color-muted-foreground)]">
              Upload a save file to back it up and restore on another machine.
            </p>
          ) : (
            attachments.saves.map((save) => (
              <AttachmentRow
                key={save.id}
                filename={save.filename}
                meta={formatBytes(save.size)}
                onDownload={() => handleDownloadSave(save.id, save.filename)}
                onDelete={() => handleDeleteSave(save.id, save.filename)}
              />
            ))
          )}
        </section>

        <section className="space-y-3">
          <SectionHeader title="Patches">
            <ArchiveUpload
              gameId={gameId}
              kind="patch"
              variant="outline"
              size="sm"
              label="Upload patch"
              className="w-full sm:w-auto"
              onComplete={onUpdated}
            />
          </SectionHeader>
          {attachments.patches.length === 0 ? (
            <p className="text-sm text-[var(--color-muted-foreground)]">
              Translations, uncensor patches, and other add-ons.
            </p>
          ) : (
            attachments.patches.map((patch) => (
              <AttachmentRow
                key={patch.id}
                filename={patch.filename}
                meta={formatBytes(patch.size)}
                onDownload={() => handleDownloadPatch(patch.id, patch.filename)}
                onDelete={() => handleDeletePatch(patch.id, patch.filename)}
              />
            ))
          )}
        </section>
      </CardContent>
    </Card>
  );
}
