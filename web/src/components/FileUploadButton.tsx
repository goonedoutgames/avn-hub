import { useId, useRef, useState, type ChangeEvent, type DragEvent } from "react";
import { FolderOpen, Upload } from "lucide-react";

type Props = {
  label?: string;
  hint?: string;
  accept?: string;
  disabled?: boolean;
  onFile: (file: File) => void | Promise<void>;
};

export function FileUploadButton({
  label = "Choose file",
  hint = "Click Browse or drop a file here",
  accept,
  disabled,
  onFile,
}: Props) {
  const inputId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState<string | null>(null);

  const handleFile = async (file: File | undefined) => {
    if (!file || disabled || busy) return;
    setName(file.name);
    setBusy(true);
    try {
      await onFile(file);
    } finally {
      setBusy(false);
      if (inputRef.current) inputRef.current.value = "";
    }
  };

  const onChange = (e: ChangeEvent<HTMLInputElement>) => {
    void handleFile(e.target.files?.[0]);
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragging(false);
    void handleFile(e.dataTransfer.files?.[0]);
  };

  return (
    <div
      className={`upload-zone ${dragging ? "upload-zone-active" : ""} ${disabled || busy ? "opacity-60" : ""}`}
      onDragEnter={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragOver={(e) => e.preventDefault()}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      <input
        ref={inputRef}
        id={inputId}
        type="file"
        className="sr-only"
        accept={accept}
        disabled={disabled || busy}
        onChange={onChange}
      />
      <div className="upload-zone-icon" aria-hidden>
        <Upload className="h-5 w-5" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-semibold">{busy ? "Uploading…" : label}</div>
        <div className="muted truncate text-xs">{name ?? hint}</div>
      </div>
      <label htmlFor={inputId} className="btn btn-primary btn-browse shrink-0 cursor-pointer">
        <FolderOpen className="h-4 w-4" aria-hidden />
        Browse
      </label>
    </div>
  );
}
