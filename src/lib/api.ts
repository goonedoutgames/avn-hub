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

export const isWebMode = () => !isTauri();

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    credentials: "include",
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  });
  if (res.status === 401) {
    throw new Error("Unauthorized");
  }
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error || res.statusText);
  }
  if (res.status === 204) {
    return undefined as T;
  }
  return res.json();
}

export const api = {
  async getAuthStatus(): Promise<import("./types").AuthStatus> {
    return apiFetch("/auth/status");
  },

  async login(username: string, password: string): Promise<void> {
    await apiFetch("/auth/login", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    });
  },

  async logout(): Promise<void> {
    await apiFetch("/auth/logout", { method: "POST" });
  },

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

  async listGames(params: import("./types").LibraryListParams = {}): Promise<GameResponse[]> {
    const {
      search,
      tags,
      tagsMode,
      playStatus,
      minF95Rating,
      maxF95Rating,
      minUserRating,
      maxUserRating,
      sort,
    } = params;

    const urlParams = new URLSearchParams();
    if (search?.trim()) urlParams.set("q", search.trim());
    if (tags?.trim()) urlParams.set("tags", tags.trim());
    if (tagsMode === "or") urlParams.set("tags_mode", "or");
    if (playStatus && playStatus.length > 0) {
      urlParams.set("play_status", playStatus.join(","));
    }
    if (minF95Rating != null) urlParams.set("min_f95_rating", String(minF95Rating));
    if (maxF95Rating != null) urlParams.set("max_f95_rating", String(maxF95Rating));
    if (minUserRating != null) urlParams.set("min_user_rating", String(minUserRating));
    if (maxUserRating != null) urlParams.set("max_user_rating", String(maxUserRating));
    if (sort) urlParams.set("sort", sort);
    const query = urlParams.toString();

    if (isTauri()) {
      const raw = await invoke<unknown[]>("list_games", {
        search: search?.trim() || null,
        tags: tags?.trim() || null,
        tags_mode: tagsMode ?? null,
        play_status:
          playStatus && playStatus.length > 0 ? playStatus.join(",") : null,
        min_f95_rating: minF95Rating ?? null,
        max_f95_rating: maxF95Rating ?? null,
        min_user_rating: minUserRating ?? null,
        max_user_rating: maxUserRating ?? null,
        sort: sort ?? null,
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
    const emptyAttachments = (): import("./types").GameAttachments => ({
      platform_archives: [],
      saves: [],
      patches: [],
    });
    const normalize = (data: import("./types").GameDetail) => ({
      ...data,
      cover_full_url: data.cover_full_url ?? data.cover_url,
      is_custom_cover: data.is_custom_cover ?? false,
      screenshots: normalizeScreenshots(data.screenshots),
      attachments: data.attachments ?? emptyAttachments(),
    });
    if (isTauri()) {
      const data = await invoke<import("./types").GameDetail>("get_game_detail", { id });
      return normalize(data);
    }
    const data = await apiFetch<import("./types").GameDetail>(`/games/${id}/detail`);
    return normalize(data);
  },

  async checkGameVersion(id: number): Promise<import("./types").VersionCheckResult> {
    if (isTauri()) return invoke("check_game_version", { id });
    return apiFetch(`/games/${id}/check-version`, { method: "POST" });
  },

  async unmatchGame(id: number): Promise<void> {
    if (isTauri()) {
      await invoke("unmatch_game", { id });
      return;
    }
    await apiFetch(`/games/${id}/unmatch`, { method: "POST" });
  },

  async deleteArchive(gameId: number): Promise<void> {
    if (isTauri()) {
      await invoke("delete_archive", { gameId });
      return;
    }
    await apiFetch(`/games/${gameId}/archive`, { method: "DELETE" });
  },

  async deletePlatformArchive(gameId: number, archiveId: number): Promise<void> {
    if (isTauri()) {
      await invoke("delete_platform_archive", { archiveId });
      return;
    }
    await apiFetch(`/games/${gameId}/archives/${archiveId}`, { method: "DELETE" });
  },

  async setDefaultPlatformArchive(
    gameId: number,
    archiveId: number,
  ): Promise<GameResponse> {
    if (isTauri()) {
      const raw = await invoke<unknown>("set_default_platform_archive", { archiveId });
      const game = normalizeGameResponse(raw);
      if (!game) throw new Error("Invalid response");
      return game;
    }
    const raw = await apiFetch<unknown>(
      `/games/${gameId}/archives/${archiveId}/default`,
      { method: "POST" },
    );
    const game = normalizeGameResponse(raw);
    if (!game) throw new Error("Invalid response");
    return game;
  },

  async deleteGameSave(gameId: number, saveId: number): Promise<void> {
    if (isTauri()) {
      await invoke("delete_game_save", { saveId });
      return;
    }
    await apiFetch(`/games/${gameId}/saves/${saveId}`, { method: "DELETE" });
  },

  async deleteGamePatch(gameId: number, patchId: number): Promise<void> {
    if (isTauri()) {
      await invoke("delete_game_patch", { patchId });
      return;
    }
    await apiFetch(`/games/${gameId}/patches/${patchId}`, { method: "DELETE" });
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

  async resolveF95Thread(url: string): Promise<F95SearchResult> {
    if (isTauri()) return invoke("resolve_f95_thread", { url });
    return apiFetch(`/search/f95/thread?url=${encodeURIComponent(url)}`);
  },

  async suggestMatches(
    archiveId?: number,
    archivePath?: string,
  ): Promise<F95SearchResult[]> {
    if (isTauri()) {
      return invoke("suggest_matches", { archiveId: archiveId ?? null, archivePath: archivePath ?? null });
    }
    const params = new URLSearchParams();
    if (archiveId != null) params.set("archive_id", String(archiveId));
    if (archivePath) params.set("path", archivePath);
    return apiFetch(`/archives/suggest?${params.toString()}`);
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

  async resetGameCover(id: number): Promise<GameResponse> {
    if (isTauri()) {
      const raw = await invoke<unknown>("reset_game_cover", { id });
      const game = normalizeGameResponse(raw);
      if (!game) throw new Error("Invalid cover response");
      return game;
    }
    const raw = await apiFetch<unknown>(`/games/${id}/cover/reset`, {
      method: "POST",
    });
    const game = normalizeGameResponse(raw);
    if (!game) throw new Error("Invalid cover response");
    return game;
  },

  async updateGameUserData(
    id: number,
    req: import("./types").UpdateGameUserDataRequest,
  ): Promise<GameResponse> {
    if (isTauri()) {
      const raw = await invoke<unknown>("update_game_user_data", { id, req });
      const game = normalizeGameResponse(raw);
      if (!game) throw new Error("Invalid update response");
      return game;
    }
    const raw = await apiFetch<unknown>(`/games/${id}/user-data`, {
      method: "PUT",
      body: JSON.stringify(req),
    });
    const game = normalizeGameResponse(raw);
    if (!game) throw new Error("Invalid update response");
    return game;
  },

  async getStorageStats(): Promise<import("./types").StorageStats> {
    if (isTauri()) return invoke("get_storage_stats");
    return apiFetch("/settings/storage");
  },

  async getMigrationStatus(): Promise<import("./types").MigrationStatus> {
    if (isTauri()) return invoke("get_migration_status");
    return apiFetch("/archives/migration");
  },

  async reorganizeArchives(): Promise<import("./types").ReorganizeResult> {
    if (isTauri()) return invoke("reorganize_archives");
    return apiFetch("/archives/reorganize", { method: "POST" });
  },

  async assignArchivePlatform(
    gameId: number,
    archiveId: number,
    platform: import("./types").Platform,
    reorganize = true,
  ): Promise<import("./types").GamePlatformArchive> {
    if (isTauri()) {
      return invoke("assign_archive_platform", {
        archiveId,
        platform,
        reorganize,
      });
    }
    return apiFetch(`/games/${gameId}/archives/${archiveId}/platform`, {
      method: "PUT",
      body: JSON.stringify({ platform, reorganize }),
    });
  },

  async downloadGame(
    gameId: number,
    filename: string,
    archiveId?: number,
  ): Promise<void> {
    if (isTauri()) {
      await invoke("download_game", { gameId, archiveId: archiveId ?? null });
      return;
    }

    const query =
      archiveId != null ? `?archive_id=${encodeURIComponent(String(archiveId))}` : "";
    const res = await fetch(`/api/games/${gameId}/download${query}`, {
      credentials: "include",
    });
    if (!res.ok) throw new Error("Download failed");
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  },

  async downloadGameSave(gameId: number, saveId: number, filename: string): Promise<void> {
    const res = await fetch(`/api/games/${gameId}/saves/${saveId}/download`, {
      credentials: "include",
    });
    if (!res.ok) throw new Error("Download failed");
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  },

  async downloadGamePatch(
    gameId: number,
    patchId: number,
    filename: string,
  ): Promise<void> {
    const res = await fetch(`/api/games/${gameId}/patches/${patchId}/download`, {
      credentials: "include",
    });
    if (!res.ok) throw new Error("Download failed");
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  },

  async downloadPlatformArchive(
    gameId: number,
    archiveId: number,
    filename: string,
  ): Promise<void> {
    if (isTauri()) {
      await invoke("download_game", { gameId, archiveId });
      return;
    }
    const res = await fetch(`/api/games/${gameId}/archives/${archiveId}/download`, {
      credentials: "include",
    });
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
