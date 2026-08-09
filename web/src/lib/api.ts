import type {
  AuthMe,
  CatalogPage,
  CatalogPreview,
  CatalogTag,
  GameDetail,
  GamePatch,
  GameSave,
  GameSummary,
  LibraryPlatform,
  LibraryTag,
  SettingsView,
  StorageStats,
  VersionCheckResult,
} from "./types";

const TOKEN_KEY = "avn_hub_token";

let apiBaseCache: string | null = null;

export function getStoredToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setStoredToken(token: string | null) {
  if (token) localStorage.setItem(TOKEN_KEY, token);
  else localStorage.removeItem(TOKEN_KEY);
}

export async function resolveApiBase(): Promise<string> {
  if (apiBaseCache) return apiBaseCache;

  const envBase = import.meta.env.VITE_API_BASE as string | undefined;
  if (envBase) {
    apiBaseCache = envBase.replace(/\/$/, "");
    return apiBaseCache;
  }

  try {
    const res = await fetch("/config.json", { cache: "no-store" });
    if (res.ok) {
      const cfg = (await res.json()) as { apiBase?: string };
      if (cfg.apiBase) {
        let base = cfg.apiBase.replace(/\/$/, "");
        // Docker default writes http://127.0.0.1:8080 into config.json. That only
        // works on the server machine — remote browsers would NetworkError.
        try {
          const configured = new URL(base, window.location.origin);
          const loopback =
            configured.hostname === "127.0.0.1" ||
            configured.hostname === "localhost";
          const pageLoopback =
            window.location.hostname === "127.0.0.1" ||
            window.location.hostname === "localhost";
          if (loopback && !pageLoopback) {
            base = `${window.location.protocol}//${window.location.hostname}:8080`;
          }
        } catch {
          // keep as-is
        }
        apiBaseCache = base;
        return apiBaseCache;
      }
    }
  } catch {
    // fall through
  }

  // Dev default: Vite proxies /api, or same host different port
  if (import.meta.env.DEV) {
    apiBaseCache = "";
    return apiBaseCache;
  }

  apiBaseCache = `${window.location.protocol}//${window.location.hostname}:8080`;
  return apiBaseCache;
}

async function apiUrl(path: string): Promise<string> {
  const base = await resolveApiBase();
  return `${base}${path.startsWith("/") ? path : `/${path}`}`;
}

export function mediaUrl(path: string | null | undefined): string | null {
  if (!path) return null;
  if (path.startsWith("http://") || path.startsWith("https://")) return path;
  const base = apiBaseCache ?? "";
  let url = `${base}${path.startsWith("/") ? path : `/${path}`}`;
  const token = getStoredToken();
  if (token) {
    url += (url.includes("?") ? "&" : "?") + `token=${encodeURIComponent(token)}`;
  }
  return url;
}

async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  const token = getStoredToken();
  if (token) headers.set("Authorization", `Bearer ${token}`);
  if (init.body && !(init.body instanceof FormData) && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const url = await apiUrl(path);
  let res: Response;
  try {
    res = await fetch(url, { ...init, headers });
  } catch (err) {
    const detail = err instanceof Error ? err.message : "Network request failed";
    throw new Error(
      `Network error talking to the API (${detail}). The request may have timed out — try again.`,
    );
  }
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  let data: { error?: string } | null = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    if (res.status === 502 || res.status === 504) {
      throw new Error(
        `API gateway error (${res.status}). Cloudflare/SWAG could not reach the app in time — check the container is up and proxy timeouts are ≥120s for /api.`,
      );
    }
    throw new Error(
      res.ok ? "Invalid JSON from API" : `${res.status} ${res.statusText || "Request failed"}`,
    );
  }
  if (!res.ok) {
    throw new Error(data?.error || res.statusText || "Request failed");
  }
  return data as T;
}

export const api = {
  me: () => request<AuthMe>("/api/v1/auth/me"),
  login: (password: string) =>
    request<{ token: string }>("/api/v1/auth/login", {
      method: "POST",
      body: JSON.stringify({ password }),
    }),
  logout: () => request<{ ok: boolean }>("/api/v1/auth/logout", { method: "POST" }),

  settings: () => request<SettingsView>("/api/v1/settings"),
  updateSettings: (body: Record<string, unknown>) =>
    request<SettingsView>("/api/v1/settings", {
      method: "PUT",
      body: JSON.stringify(body),
    }),
  f95Login: (username: string, password: string) =>
    request<{ ok: boolean; message: string }>("/api/v1/settings/f95/login", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    }),
  f95Cookies: (cookies: string) =>
    request<{ ok: boolean; message: string }>("/api/v1/settings/f95/cookies", {
      method: "POST",
      body: JSON.stringify({ cookies }),
    }),
  storage: () => request<StorageStats>("/api/v1/settings/storage"),
  purgeMedia: () =>
    request<{ ok: boolean }>("/api/v1/settings/media/purge", { method: "POST" }),

  searchCatalog: (params: {
    q?: string;
    creator?: string;
    page?: number;
    rows?: number;
    sort?: string;
    date?: number;
    tag_mode?: string;
    tags?: string;
    notags?: string;
    prefixes?: string;
  } = {}) => {
    const qs = new URLSearchParams();
    if (params.q) qs.set("q", params.q);
    if (params.creator) qs.set("creator", params.creator);
    qs.set("page", String(params.page ?? 1));
    qs.set("rows", String(params.rows ?? 90));
    qs.set("sort", params.sort ?? "date");
    if (params.date && params.date > 0) qs.set("date", String(params.date));
    if (params.tag_mode) qs.set("tag_mode", params.tag_mode);
    if (params.tags) qs.set("tags", params.tags);
    if (params.notags) qs.set("notags", params.notags);
    if (params.prefixes) qs.set("prefixes", params.prefixes);
    return request<CatalogPage>(`/api/v1/catalog/search?${qs}`);
  },
  browseCatalog: (page = 1, sort = "date", rows = 90) =>
    request<CatalogPage>(
      `/api/v1/catalog/browse?page=${page}&sort=${encodeURIComponent(sort)}&rows=${rows}`,
    ),
  catalogTags: (q?: string, limit = 500) => {
    const qs = new URLSearchParams();
    if (q) qs.set("q", q);
    qs.set("limit", String(limit));
    return request<CatalogTag[]>(`/api/v1/catalog/tags?${qs}`);
  },
  previewCatalog: (input: string, titleHint?: string) => {
    const qs = new URLSearchParams({ input });
    if (titleHint?.trim()) qs.set("title_hint", titleHint.trim());
    return request<CatalogPreview>(`/api/v1/catalog/preview?${qs}`);
  },

  library: (params: Record<string, string | undefined> = {}) => {
    const qs = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) {
      if (v) qs.set(k, v);
    }
    const q = qs.toString();
    return request<GameSummary[]>(`/api/v1/library${q ? `?${q}` : ""}`);
  },
  libraryTags: () => request<LibraryTag[]>("/api/v1/library/tags"),
  libraryPlatforms: () => request<LibraryPlatform[]>("/api/v1/library/platforms"),
  addGame: (input: string, titleHint?: string) =>
    request<GameDetail>("/api/v1/library/add", {
      method: "POST",
      body: JSON.stringify({
        input,
        ...(titleHint?.trim() ? { title_hint: titleHint.trim() } : {}),
      }),
    }),
  checkAllUpdates: () =>
    request<VersionCheckResult[]>("/api/v1/library/check-updates", {
      method: "POST",
    }),

  game: (id: number) => request<GameDetail>(`/api/v1/games/${id}`),
  patchGame: (id: number, body: Record<string, unknown>) =>
    request<GameDetail>(`/api/v1/games/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),
  deleteGame: (id: number) =>
    request<void>(`/api/v1/games/${id}`, { method: "DELETE" }),
  refreshGame: (id: number) =>
    request<GameDetail>(`/api/v1/games/${id}/refresh`, { method: "POST" }),
  checkVersion: (id: number) =>
    request<VersionCheckResult>(`/api/v1/games/${id}/check-version`, {
      method: "POST",
    }),
  setCover: (id: number, screenshot_index: number) =>
    request<GameDetail>(`/api/v1/games/${id}/cover`, {
      method: "POST",
      body: JSON.stringify({ screenshot_index }),
    }),
  resetCover: (id: number) =>
    request<GameDetail>(`/api/v1/games/${id}/cover/reset`, { method: "POST" }),

  uploadSave: async (id: number, file: File) => {
    const form = new FormData();
    form.append("file", file);
    return request<GameSave>(`/api/v1/games/${id}/saves`, {
      method: "POST",
      body: form,
    });
  },
  downloadSaveUrl: async (gameId: number, saveId: number) =>
    apiUrl(`/api/v1/games/${gameId}/saves/${saveId}`),
  deleteSave: (gameId: number, saveId: number) =>
    request<void>(`/api/v1/games/${gameId}/saves/${saveId}`, {
      method: "DELETE",
    }),

  uploadPatch: async (id: number, file: File, description?: string) => {
    const form = new FormData();
    form.append("file", file);
    if (description) form.append("description", description);
    return request<GamePatch>(`/api/v1/games/${id}/patches`, {
      method: "POST",
      body: form,
    });
  },
  downloadPatchUrl: async (gameId: number, patchId: number) =>
    apiUrl(`/api/v1/games/${gameId}/patches/${patchId}`),
  deletePatch: (gameId: number, patchId: number) =>
    request<void>(`/api/v1/games/${gameId}/patches/${patchId}`, {
      method: "DELETE",
    }),
};

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

/** Example: January 30th 2026 11:49 PM (local time). */
export function formatFriendlyDate(raw?: string | null): string {
  if (!raw) return "—";
  const d = new Date(raw);
  if (Number.isNaN(d.getTime())) return raw;
  const day = d.getDate();
  const suffix =
    day % 10 === 1 && day !== 11
      ? "st"
      : day % 10 === 2 && day !== 12
        ? "nd"
        : day % 10 === 3 && day !== 13
          ? "rd"
          : "th";
  const month = d.toLocaleString(undefined, { month: "long" });
  const time = d.toLocaleString(undefined, { hour: "numeric", minute: "2-digit", hour12: true });
  return `${month} ${day}${suffix} ${d.getFullYear()} ${time}`;
}
