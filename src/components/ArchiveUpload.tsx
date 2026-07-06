import { useRef, useState } from "react";
import { Upload } from "lucide-react";
import * as tus from "tus-js-client";
import { isWebMode } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import { formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";

const ARCHIVE_ACCEPT = ".zip,.rar,.7z,.bz2";
const UPLOAD_CHUNK_SIZE = 8 * 1024 * 1024;

interface ArchiveUploadProps {
  /** When set, replaces the archive file for this game and keeps metadata. */
  replaceGameId?: number;
  onComplete?: () => void;
  variant?: "default" | "secondary" | "outline";
  label?: string;
  uploadingLabel?: string;
}

export function ArchiveUpload({
  replaceGameId,
  onComplete,
  variant = "secondary",
  label,
  uploadingLabel,
}: ArchiveUploadProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const { startTask, updateTask, endTask } = useTasks();
  const [uploading, setUploading] = useState(false);
  const activeUploadRef = useRef<tus.Upload | null>(null);

  if (!isWebMode()) return null;

  const isReplace = replaceGameId != null;
  const buttonLabel = label ?? (isReplace ? "Replace archive" : "Upload archive");
  const busyLabel = uploadingLabel ?? (isReplace ? "Replacing…" : "Uploading…");

  const uploadFile = (file: File) => {
    const taskId = `upload-${Date.now()}`;
    const taskLabel = isReplace
      ? `Replacing archive with ${file.name}`
      : `Uploading ${file.name}`;
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
    };
    if (isReplace) {
      metadata.game_id = String(replaceGameId);
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
          detail: isReplace
            ? "Archive replaced"
            : "Upload complete — run Scan if the file is not listed",
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
    if (isReplace) {
      const ok = window.confirm(
        `Replace the current archive with “${files[0].name}”? Metadata (title, cover, tags) will be kept.`,
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
        accept={ARCHIVE_ACCEPT}
        className="hidden"
        onChange={(e) => handleFiles(e.target.files)}
      />
      <Button
        type="button"
        variant={variant}
        disabled={uploading}
        onClick={() => inputRef.current?.click()}
      >
        <Upload className="h-4 w-4" />
        {uploading ? busyLabel : buttonLabel}
      </Button>
    </div>
  );
}
