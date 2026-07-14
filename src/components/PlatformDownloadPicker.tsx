import { Download, X } from "lucide-react";
import type { GamePlatformArchive } from "@/lib/types";
import { platformLabel } from "@/lib/types";
import { formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";

interface PlatformDownloadPickerProps {
  gameTitle: string;
  archives: GamePlatformArchive[];
  onDownload: (archive: GamePlatformArchive) => void;
  onClose: () => void;
}

export function PlatformDownloadPicker({
  gameTitle,
  archives,
  onDownload,
  onClose,
}: PlatformDownloadPickerProps) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 p-4 sm:items-center"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-4 shadow-lg"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-labelledby="platform-download-title"
      >
        <div className="mb-4 flex items-start justify-between gap-3">
          <div>
            <h2 id="platform-download-title" className="font-semibold">
              Choose platform
            </h2>
            <p className="text-sm text-[var(--color-muted-foreground)]">{gameTitle}</p>
          </div>
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="space-y-2">
          {archives.map((archive) => (
            <button
              key={archive.id}
              type="button"
              className="flex w-full items-center gap-3 rounded-lg border border-[var(--color-border)] p-3 text-left transition-colors hover:border-[var(--color-primary)] hover:bg-[var(--color-accent)]"
              onClick={() => onDownload(archive)}
            >
              <Download className="h-4 w-4 shrink-0 text-[var(--color-muted-foreground)]" />
              <div className="min-w-0 flex-1">
                <p className="font-medium">{platformLabel(archive.platform)}</p>
                <p className="truncate text-xs text-[var(--color-muted-foreground)]">
                  {archive.filename} · {formatBytes(archive.size)}
                </p>
              </div>
              {archive.is_default && (
                <span className="shrink-0 text-[10px] text-[var(--color-muted-foreground)]">
                  default
                </span>
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
