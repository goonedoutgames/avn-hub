import { forwardRef, useImperativeHandle, useRef, useState } from "react";
import { Upload } from "lucide-react";
import * as tus from "tus-js-client";
import { isWebMode } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import type { Platform, UploadKind } from "@/lib/types";
import { cn, formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export interface ArchiveUploadHandle {
  open: () => void;
}

const ARCHIVE_ACCEPT = ".zip,.rar,.7z,.bz2,.apk,.xapk,.apks";
const PATCH_ACCEPT = ".zip,.rar,.7z,.bz2,.patch,.ppk,.exe";
const UPLOAD_CHUNK_SIZE = 8 * 1024 * 1024;

interface ArchiveUploadProps {
  /** When set, attaches upload to this game (archives, saves, patches). */
  gameId?: number;
  /** Replaces a specific platform archive row. */
  replaceArchiveId?: number;
  kind?: UploadKind;
  platform?: Platform;
  onComplete?: () => void;
  variant?: "default" | "secondary" | "outline";
  label?: string;
  uploadingLabel?: string;
  className?: string;
  size?: "default" | "sm" | "lg" | "icon";
  /** @deprecated use gameId */
  replaceGameId?: number;
}

export const ArchiveUpload = forwardRef<ArchiveUploadHandle, ArchiveUploadProps>(
  function ArchiveUpload(
    {
      gameId,
      replaceArchiveId,
      kind = "archive",
      platform,
      onComplete,
      variant = "secondary",
      label,
      uploadingLabel,
      className,
      size,
      replaceGameId,
    },
    ref,
  ) {
  const resolvedGameId = gameId ?? replaceGameId;
  const inputRef = useRef<HTMLInputElement>(null);
  const { startTask, updateTask, endTask } = useTasks();
  const [uploading, setUploading] = useState(false);
  const activeUploadRef = useRef<tus.Upload | null>(null);

  useImperativeHandle(ref, () => ({
    open: () => {
      if (!uploading) inputRef.current?.click();
    },
  }));

  if (!isWebMode()) return null;

  const isReplace = replaceArchiveId != null || resolvedGameId != null;
  const accept =
    kind === "save" ? undefined : kind === "patch" ? PATCH_ACCEPT : ARCHIVE_ACCEPT;

  const defaultLabel =
    kind === "save"
      ? "Upload save"
      : kind === "patch"
        ? "Upload patch"
        : isReplace
          ? "Upload platform archive"
          : "Upload archive";

  const defaultBusyLabel =
    kind === "save"
      ? "Uploading save…"
      : kind === "patch"
        ? "Uploading patch…"
        : isReplace
          ? "Uploading…"
          : "Uploading…";

  const buttonLabel = label ?? defaultLabel;
  const busyLabel = uploadingLabel ?? defaultBusyLabel;

  const uploadFile = (file: File) => {
    const taskId = `upload-${Date.now()}`;
    const taskLabel = `${buttonLabel}: ${file.name}`;
    startTask(taskId, taskLabel, "Preparing…");

    setUploading(true);
    const finish = (delayMs = 0) => {
      window.setTimeout(() => {
        endTask(taskId);
        activeUploadRef.current = null;
        setUploading(false);
      }, delayMs);
    };

    const metadata: Record<string, string> = {
      filename: file.name,
      filetype: file.type || "application/octet-stream",
      kind,
    };
    if (resolvedGameId != null) {
      metadata.game_id = String(resolvedGameId);
    }
    if (platform && kind === "archive") {
      metadata.platform = platform;
    }
    if (replaceArchiveId != null) {
      metadata.replace_archive_id = String(replaceArchiveId);
    }

    const upload = new tus.Upload(file, {
      endpoint: "/api/tus",
      chunkSize: UPLOAD_CHUNK_SIZE,
      retryDelays: [0, 2000, 5000, 10000, 20000],
      storeFingerprintForResuming: true,
      removeFingerprintOnSuccess: true,
      metadata,
      onBeforeRequest: (req) => {
        const xhr = req.getUnderlyingObject() as XMLHttpRequest;
        xhr.withCredentials = true;
      },
      onError: (err) => {
        const detailed = err as tus.DetailedError;
        const message =
          detailed.message ||
          (detailed.originalResponse
            ? `Server error ${detailed.originalResponse.getStatus()}`
            : "Upload failed");
        updateTask(taskId, { detail: message, progress: undefined });
        finish(6000);
      },
      onProgress: (bytesUploaded, bytesTotal) => {
        const progress =
          bytesTotal > 0 ? Math.round((bytesUploaded / bytesTotal) * 100) : 0;
        updateTask(taskId, {
          detail: `${formatBytes(bytesUploaded)} / ${formatBytes(bytesTotal)}`,
          progress,
        });
      },
      onSuccess: () => {
        updateTask(taskId, {
          detail:
            kind === "archive" && !resolvedGameId
              ? "Upload complete — run Scan if the file is not listed"
              : "Upload complete",
          progress: 100,
        });
        finish(2500);
        onComplete?.();
      },
    });

    activeUploadRef.current = upload;

    upload
      .findPreviousUploads()
      .then((previous) => {
        if (previous.length > 0) {
          updateTask(taskId, { detail: "Resuming previous upload…" });
          upload.resumeFromPreviousUpload(previous[0]);
        }
        upload.start();
      })
      .catch((err) => {
        updateTask(taskId, {
          detail: err instanceof Error ? err.message : "Failed to start upload",
        });
        finish(6000);
      });
  };

  const handleFiles = (files: FileList | null) => {
    if (!files?.length || activeUploadRef.current) return;
    if (replaceArchiveId != null) {
      const ok = window.confirm(
        `Replace this platform archive with “${files[0].name}”?`,
      );
      if (!ok) {
        if (inputRef.current) inputRef.current.value = "";
        return;
      }
    }
    uploadFile(files[0]);
    if (inputRef.current) inputRef.current.value = "";
  };

  return (
    <div>
      <input
        ref={inputRef}
        type="file"
        accept={accept}
        className="hidden"
        onChange={(e) => handleFiles(e.target.files)}
      />
      <Button
        type="button"
        variant={variant}
        size={size}
        disabled={uploading}
        className={cn(className)}
        onClick={() => inputRef.current?.click()}
      >
        <Upload className="h-4 w-4" />
        {uploading ? busyLabel : buttonLabel}
      </Button>
    </div>
  );
},
);
