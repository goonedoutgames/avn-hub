import { invoke } from "@tauri-apps/api/core";
import type {
  ArchiveEntry,
  F95LoginRequest,
  F95LoginResult,
  F95SearchResult,
  GameResponse,
  MatchRequest,
  ScanResult,
  Settings,
  UpdateSettingsRequest,
} from "./types";
import { normalizeGameList, normalizeGameResponse } from "./game-response";

const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function normalizeScreenshots(
  raw: import("./types").ScreenshotItem[] | string[] | unknown,
): import("./types").ScreenshotItem[] {
  if (!Array.isArray(raw)) return [];
  if (raw.length === 0) return [];
  if (typeof raw[0] === "string") {
    return (raw as string[]).map((url) => ({
      full_url: url,
      cached_url: url,
    }));
  }
  return raw as import("./types").ScreenshotItem[];
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error || res.statusText);
  }
  return res.json();
}

export const api = {
  async getSettings(): Promise<Settings> {
    if (isTauri()) return invoke("get_settings");
    return apiFetch("/settings");
  },

  async updateSettings(req: UpdateSettingsRequest): Promise<Settings> {
    if (isTauri()) return invoke("update_settings", { req });
    return apiFetch("/settings", {
      method: "PUT",
      body: JSON.stringify(req),
    });
  },

  async purgeMediaCache(): Promise<void> {
    if (isTauri()) {
      await invoke("purge_media_cache");
      return;
    }
    await apiFetch("/settings/purge-media", { method: "POST" });
  },

  async f95Login(req: F95LoginRequest = {}): Promise<F95LoginResult> {
    if (isTauri()) return invoke("f95_login", { req });
    return apiFetch("/f95/login", {
      method: "POST",
      body: JSON.stringify(req),
    });
  },

  async listGames(
    search?: string,
    tags?: string,
    tagsMode?: import("./types").TagFilterMode,
  ): Promise<GameResponse[]> {
    const params = new URLSearchParams();
    if (search?.trim()) params.set("q", search.trim());
    if (tags?.trim()) params.set("tags", tags.trim());
    if (tagsMode === "or") params.set("tags_mode", "or");
    const query = params.toString();

    if (isTauri()) {
      const raw = await invoke<unknown[]>("list_games", {
        search: search?.trim() || null,
        tags: tags?.trim() || null,
        tags_mode: tagsMode ?? null,
      });
      return normalizeGameList(raw);
    }
    const q = query ? `?${query}` : "";
    const raw = await apiFetch<unknown[]>(`/games${q}`);
    return normalizeGameList(raw);
  },

  async listLibraryTags(): Promise<import("./types").LibraryTag[]> {
    if (isTauri()) return invoke("list_library_tags");
    return apiFetch("/games/tags");
  },

  async getGame(id: number): Promise<GameResponse> {
    if (isTauri()) {
      const raw = await invoke<unknown>("get_game", { id });
      const game = normalizeGameResponse(raw);
      if (!game) throw new Error("Invalid game response");
      return game;
    }
    const raw = await apiFetch<unknown>(`/games/${id}`);
    const game = normalizeGameResponse(raw);
    if (!game) throw new Error("Invalid game response");
    return game;
  },

  async getGameDetail(id: number): Promise<import("./types").GameDetail> {
    const normalize = (data: import("./types").GameDetail) => ({
      ...data,
      cover_full_url: data.cover_full_url ?? data.cover_url,
      screenshots: normalizeScreenshots(data.screenshots),
    });
    if (isTauri()) {
      const data = await invoke<import("./types").GameDetail>("get_game_detail", { id });
      return normalize(data);
    }
    const data = await apiFetch<import("./types").GameDetail>(`/games/${id}/detail`);
    return normalize(data);
  },

  async unmatchGame(id: number): Promise<void> {
    if (isTauri()) {
      await invoke("unmatch_game", { id });
      return;
    }
    await apiFetch(`/games/${id}/unmatch`, { method: "POST" });
  },

  async listArchives(): Promise<ArchiveEntry[]> {
    if (isTauri()) return invoke("list_archives");
    return apiFetch("/archives");
  },

  async scanArchives(): Promise<ScanResult> {
    if (isTauri()) return invoke("scan_archives");
    return apiFetch("/archives/scan", { method: "POST" });
  },

  async searchF95(query: string, page = 1): Promise<F95SearchResult[]> {
    if (isTauri()) return invoke("search_f95", { query, page });
    return apiFetch(`/search/f95?q=${encodeURIComponent(query)}&page=${page}`);
  },

  async suggestMatches(archivePath: string): Promise<F95SearchResult[]> {
    if (isTauri()) return invoke("suggest_matches", { archivePath });
    return apiFetch(
      `/archives/suggest?path=${encodeURIComponent(archivePath)}`,
    );
  },

  async matchArchive(req: MatchRequest): Promise<GameResponse> {
    if (isTauri()) {
      const raw = await invoke<unknown>("match_archive", { req });
      const game = normalizeGameResponse(raw);
      if (!game) throw new Error("Invalid match response");
      return game;
    }
    const raw = await apiFetch<unknown>("/archives/match", {
      method: "POST",
      body: JSON.stringify(req),
    });
    const game = normalizeGameResponse(raw);
    if (!game) throw new Error("Invalid match response");
    return game;
  },

  async setGameCover(id: number, screenshotIndex: number): Promise<GameResponse> {
    if (isTauri()) {
      const raw = await invoke<unknown>("set_game_cover", {
        id,
        screenshotIndex,
      });
      const game = normalizeGameResponse(raw);
      if (!game) throw new Error("Invalid cover response");
      return game;
    }
    const raw = await apiFetch<unknown>(`/games/${id}/cover`, {
      method: "POST",
      body: JSON.stringify({ screenshot_index: screenshotIndex }),
    });
    const game = normalizeGameResponse(raw);
    if (!game) throw new Error("Invalid cover response");
    return game;
  },

  async downloadGame(gameId: number, filename: string): Promise<void> {
    if (isTauri()) {
      await invoke("download_game", { gameId });
      return;
    }

    const res = await fetch(`/api/games/${gameId}/download`);
    if (!res.ok) throw new Error("Download failed");
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  },
};
