import { useEffect, type ReactNode } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

interface SheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  side: "left" | "right";
  title: string;
  description?: string;
  children: ReactNode;
}

export function Sheet({
  open,
  onOpenChange,
  side,
  title,
  description,
  children,
}: SheetProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onOpenChange(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onOpenChange]);

  useEffect(() => {
    document.body.style.overflow = open ? "hidden" : "";
    return () => {
      document.body.style.overflow = "";
    };
  }, [open]);

  return (
    <>
      <div
        aria-hidden={!open}
        className={cn(
          "fixed inset-0 z-50 bg-black/50 backdrop-blur-[2px] transition-opacity duration-300",
          open ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onClick={() => onOpenChange(false)}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-labelledby="sheet-title"
        aria-hidden={!open}
        className={cn(
          "fixed top-0 z-50 flex h-full w-full max-w-sm flex-col border-[var(--color-border)] bg-[var(--color-card)] shadow-2xl transition-transform duration-300 ease-out",
          side === "left"
            ? "left-0 border-r"
            : "right-0 border-l",
          open
            ? "translate-x-0"
            : side === "left"
              ? "-translate-x-full"
              : "translate-x-full",
        )}
      >
        <div className="flex items-start justify-between gap-3 border-b border-[var(--color-border)] px-5 py-4">
          <div className="min-w-0 space-y-1">
            <h2 id="sheet-title" className="text-lg font-semibold leading-tight">
              {title}
            </h2>
            {description && (
              <p className="text-sm text-[var(--color-muted-foreground)]">
                {description}
              </p>
            )}
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="shrink-0"
            onClick={() => onOpenChange(false)}
            aria-label="Close"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="flex-1 overflow-y-auto px-5 py-4">{children}</div>
      </aside>
    </>
  );
}

interface FloatingSheetButtonProps {
  side: "left" | "right";
  icon: ReactNode;
  label: string;
  active?: boolean;
  badge?: number | string;
  onClick: () => void;
}

export function FloatingSheetButton({
  side,
  icon,
  label,
  active,
  badge,
  onClick,
}: FloatingSheetButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      className={cn(
        "fixed bottom-22 z-40 flex h-12 w-12 translate-y-1/2 items-center justify-center rounded-full border border-[var(--color-border)] bg-[var(--color-card)] text-[var(--color-foreground)] shadow-lg transition-all hover:scale-105 hover:border-[var(--color-primary)] hover:shadow-xl md:bottom-10",
        side === "left" ? "left-4" : "right-4",
        active && "border-[var(--color-primary)] ring-2 ring-[var(--color-primary)]/40",
      )}
    >
      {icon}
      {badge != null && badge !== 0 && badge !== "" && (
        <span className="absolute -top-1 -right-1 flex h-5 min-w-5 items-center justify-center rounded-full bg-[var(--color-primary)] px-1 text-[10px] font-semibold text-[var(--color-primary-foreground)]">
          {badge}
        </span>
      )}
    </button>
  );
}
