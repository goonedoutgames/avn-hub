import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { api, isWebMode } from "@/lib/api";
import type { AuthStatus } from "@/lib/types";

interface AuthContextValue {
  status: AuthStatus | null;
  loading: boolean;
  refresh: () => Promise<void>;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  needsLogin: boolean;
  needsSetup: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [loading, setLoading] = useState(isWebMode());

  const refresh = useCallback(async () => {
    if (!isWebMode()) {
      setStatus({
        configured: false,
        authenticated: true,
        username: null,
      });
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      setStatus(await api.getAuthStatus());
    } catch {
      setStatus({
        configured: false,
        authenticated: true,
        username: null,
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const login = useCallback(
    async (username: string, password: string) => {
      await api.login(username, password);
      await refresh();
    },
    [refresh],
  );

  const logout = useCallback(async () => {
    await api.logout();
    await refresh();
  }, [refresh]);

  const value = useMemo<AuthContextValue>(() => {
    const configured = status?.configured ?? false;
    const authenticated = status?.authenticated ?? true;
    return {
      status,
      loading,
      refresh,
      login,
      logout,
      needsLogin: configured && !authenticated,
      needsSetup: !configured,
    };
  }, [status, loading, refresh, login, logout]);

  return (
    <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
