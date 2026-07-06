import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export interface BackgroundTask {
  id: string;
  label: string;
  detail?: string;
  progress?: number;
}

interface TaskContextValue {
  tasks: BackgroundTask[];
  startTask: (id: string, label: string, detail?: string) => void;
  updateTask: (
    id: string,
    update: { label?: string; detail?: string; progress?: number },
  ) => void;
  endTask: (id: string) => void;
  runTask: <T>(
    id: string,
    label: string,
    fn: (update: (detail?: string, progress?: number) => void) => Promise<T>,
    initialDetail?: string,
  ) => Promise<T>;
}

const TaskContext = createContext<TaskContextValue | null>(null);

export function TaskProvider({ children }: { children: ReactNode }) {
  const [tasks, setTasks] = useState<BackgroundTask[]>([]);

  const startTask = useCallback(
    (id: string, label: string, detail?: string) => {
      setTasks((prev) => {
        const rest = prev.filter((t) => t.id !== id);
        return [...rest, { id, label, detail }];
      });
    },
    [],
  );

  const updateTask = useCallback(
    (
      id: string,
      update: { label?: string; detail?: string; progress?: number },
    ) => {
      setTasks((prev) =>
        prev.map((t) => (t.id === id ? { ...t, ...update } : t)),
      );
    },
    [],
  );

  const endTask = useCallback((id: string) => {
    setTasks((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const runTask = useCallback(
    async <T,>(
      id: string,
      label: string,
      fn: (update: (detail?: string, progress?: number) => void) => Promise<T>,
      initialDetail?: string,
    ): Promise<T> => {
      startTask(id, label, initialDetail);
      const update = (detail?: string, progress?: number) => {
        updateTask(id, { detail, progress });
      };
      try {
        return await fn(update);
      } finally {
        endTask(id);
      }
    },
    [startTask, updateTask, endTask],
  );

  const value = useMemo(
    () => ({ tasks, startTask, updateTask, endTask, runTask }),
    [tasks, startTask, updateTask, endTask, runTask],
  );

  return (
    <TaskContext.Provider value={value}>{children}</TaskContext.Provider>
  );
}

export function useTasks() {
  const ctx = useContext(TaskContext);
  if (!ctx) throw new Error("useTasks must be used within TaskProvider");
  return ctx;
}
