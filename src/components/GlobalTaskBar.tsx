import { Loader2 } from "lucide-react";
import { useTasks } from "@/context/TaskContext";
import { cn } from "@/lib/utils";

export function GlobalTaskBar() {
  const { tasks } = useTasks();
  if (tasks.length === 0) return null;

  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-14 z-50 border-t border-[var(--color-border)] bg-[var(--color-card)]/95 shadow-[0_-4px_24px_rgba(0,0,0,0.35)] backdrop-blur-sm md:bottom-0">
      <div className="pointer-events-auto mx-auto max-w-7xl space-y-2 px-4 py-3">
        {tasks.map((task) => (
          <TaskRow key={task.id} task={task} />
        ))}
      </div>
    </div>
  );
}

function TaskRow({
  task,
}: {
  task: { label: string; detail?: string; progress?: number };
}) {
  const indeterminate = task.progress === undefined;

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2 text-sm">
        <Loader2 className="h-4 w-4 shrink-0 animate-spin text-[var(--color-primary)]" />
        <span className="font-medium">{task.label}</span>
        {task.detail && (
          <span className="truncate text-[var(--color-muted-foreground)]">
            — {task.detail}
          </span>
        )}
        {!indeterminate && task.progress != null && (
          <span className="ml-auto shrink-0 text-xs text-[var(--color-muted-foreground)]">
            {Math.round(task.progress)}%
          </span>
        )}
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-[var(--color-muted)]">
        <div
          className={cn(
            "h-full rounded-full bg-[var(--color-primary)] transition-all",
            indeterminate && "w-1/3 animate-pulse",
          )}
          style={
            indeterminate
              ? undefined
              : { width: `${Math.min(100, Math.max(0, task.progress!))}%` }
          }
        />
      </div>
    </div>
  );
}
