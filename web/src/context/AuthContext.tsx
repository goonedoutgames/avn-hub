import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { api, resolveApiBase, setStoredToken } from "@/lib/api";

type AuthState = {
  loading: boolean;
  configured: boolean;
  authenticated: boolean;
  refresh: () => Promise<void>;
  login: (password: string) => Promise<void>;
  logout: () => Promise<void>;
};

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [loading, setLoading] = useState(true);
  const [configured, setConfigured] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);

  const refresh = useCallback(async () => {
    await resolveApiBase();
    const me = await api.me();
    setConfigured(me.configured);
    setAuthenticated(me.authenticated);
  }, []);

  useEffect(() => {
    refresh()
      .catch(() => {
        setConfigured(false);
        setAuthenticated(false);
      })
      .finally(() => setLoading(false));
  }, [refresh]);

  const login = async (password: string) => {
    const { token } = await api.login(password);
    setStoredToken(token);
    await refresh();
  };

  const logout = async () => {
    try {
      await api.logout();
    } finally {
      setStoredToken(null);
      await refresh();
    }
  };

  return (
    <AuthContext.Provider
      value={{ loading, configured, authenticated, refresh, login, logout }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth outside provider");
  return ctx;
}
