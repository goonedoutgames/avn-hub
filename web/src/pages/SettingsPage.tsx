import { FormEvent, useEffect, useState } from "react";
import { useToast } from "@/context/ToastContext";
import { api, formatBytes } from "@/lib/api";
import type { SettingsView, StorageStats } from "@/lib/types";
import { useAuth } from "@/context/AuthContext";

export function SettingsPage() {
  const { refresh } = useAuth();
  const toast = useToast();
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [storage, setStorage] = useState<StorageStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [appPassword, setAppPassword] = useState("");
  const [f95User, setF95User] = useState("");
  const [f95Pass, setF95Pass] = useState("");
  const [cookies, setCookies] = useState("");
  const [tagClickAction, setTagClickAction] = useState<"library" | "browse">("library");

  const load = async () => {
    setError(null);
    try {
      const [s, st] = await Promise.all([api.settings(), api.storage()]);
      setSettings(s);
      setStorage(st);
      setF95User(s.f95_username ?? "");
      setTagClickAction(s.tag_click_action === "browse" ? "browse" : "library");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to load settings";
      setError(msg);
      toast.error(msg);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const saveAppPassword = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      setSettings(await api.updateSettings({ app_password: appPassword }));
      setAppPassword("");
      toast.success("App password updated");
      await refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed");
    } finally {
      setBusy(false);
    }
  };

  const removeAppPassword = async () => {
    if (!confirm("Remove app password? The API will be open until you set one again.")) return;
    try {
      setSettings(await api.updateSettings({ app_password_remove: true }));
      toast.success("App password removed");
      await refresh();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed");
    }
  };

  const loginF95 = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const res = await api.f95Login(f95User, f95Pass);
      toast.success(res.message || "Logged in to F95Zone");
      setF95Pass("");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "F95 login failed");
    } finally {
      setBusy(false);
    }
  };

  const saveCookies = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      const res = await api.f95Cookies(cookies);
      toast.success(res.message || "Cookies saved");
      setCookies("");
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Cookie save failed");
    } finally {
      setBusy(false);
    }
  };

  const saveTagClickAction = async (action: "library" | "browse") => {
    setTagClickAction(action);
    setBusy(true);
    try {
      setSettings(await api.updateSettings({ tag_click_action: action }));
      toast.success(
        action === "browse"
          ? "Tag clicks open Browse"
          : "Tag clicks filter your Library",
      );
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save");
      await load();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page mx-auto max-w-2xl">
      <div>
        <h1 className="page-title">Settings</h1>
        <p className="page-subtitle">App password, F95Zone auth, and storage.</p>
      </div>

      {error && <p className="text-sm text-[var(--danger)]">{error}</p>}

      <section className="card card-section stack">
        <div>
          <h2 className="m-0 text-base font-semibold">Tag clicks</h2>
          <p className="muted mt-1 text-sm">
            When you tap a tag on a library game’s detail page, choose where to go.
          </p>
        </div>
        <div className="grid gap-2">
          <button
            type="button"
            disabled={busy}
            className={`rounded-lg border px-3 py-2.5 text-left text-sm ${
              tagClickAction === "library"
                ? "border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_16%,transparent)]"
                : "border-[var(--border)] bg-[var(--bg-soft)]"
            }`}
            onClick={() => void saveTagClickAction("library")}
          >
            <span className="block font-medium">Filter library</span>
            <span className="muted text-xs">
              Show games you already own that share that tag.
            </span>
          </button>
          <button
            type="button"
            disabled={busy}
            className={`rounded-lg border px-3 py-2.5 text-left text-sm ${
              tagClickAction === "browse"
                ? "border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_16%,transparent)]"
                : "border-[var(--border)] bg-[var(--bg-soft)]"
            }`}
            onClick={() => void saveTagClickAction("browse")}
          >
            <span className="block font-medium">Open Browse</span>
            <span className="muted text-xs">
              Search F95Zone for that tag with default date sorting.
            </span>
          </button>
        </div>
      </section>

      <section className="card card-section stack">
        <div>
          <h2 className="m-0 text-base font-semibold">App password</h2>
          <p className="muted mt-1 text-sm">
            {settings?.app_password_set
              ? "Password is configured. Clients authenticate with a Bearer token."
              : "No password set — API is open. Set one for production."}
          </p>
        </div>
        <form onSubmit={(e) => void saveAppPassword(e)} className="toolbar">
          <input
            className="input min-w-0 flex-1"
            type="password"
            placeholder="New password"
            value={appPassword}
            onChange={(e) => setAppPassword(e.target.value)}
          />
          <button className="btn btn-primary" type="submit" disabled={busy || !appPassword}>
            Set password
          </button>
        </form>
        {settings?.app_password_set && (
          <button type="button" className="btn btn-danger self-start" onClick={() => void removeAppPassword()}>
            Remove password
          </button>
        )}
      </section>

      <section className="card card-section stack">
        <div>
          <h2 className="m-0 text-base font-semibold">F95Zone</h2>
          <p className="muted mt-1 text-sm">
            Needed for Browse, add/refresh, download links, and media. Use either
            username/password or cookies — not both. Prefer cookies if login hits 2FA or
            CAPTCHA.
          </p>
          <p className="muted mt-1 text-sm">
            Status:{" "}
            {settings?.f95_authenticated ? (
              <span className="text-[var(--ok)]">authenticated</span>
            ) : (
              <span className="text-[var(--warning)]">not authenticated</span>
            )}
            {settings?.f95_cookies_set ? " · cookies saved" : ""}
          </p>
        </div>
        <form onSubmit={(e) => void loginF95(e)} className="stack">
          <p className="m-0 text-sm font-medium">Option A — username / password</p>
          <p className="muted m-0 text-xs">
            Logs the hub into f95zone.to and stores the session. Fails when F95 requires 2FA or
            CAPTCHA.
          </p>
          <label className="block text-sm">
            <span className="field-label">Username or email</span>
            <input
              className="input"
              value={f95User}
              onChange={(e) => setF95User(e.target.value)}
            />
          </label>
          <label className="block text-sm">
            <span className="field-label">Password</span>
            <input
              className="input"
              type="password"
              value={f95Pass}
              onChange={(e) => setF95Pass(e.target.value)}
            />
          </label>
          <button className="btn btn-primary self-start" type="submit" disabled={busy}>
            Login to F95Zone
          </button>
        </form>
        <form onSubmit={(e) => void saveCookies(e)} className="stack border-t border-[var(--border)] pt-4">
          <p className="m-0 text-sm font-medium">Option B — paste browser cookies</p>
          <p className="muted m-0 text-xs">
            1) Log in at f95zone.to in your browser. 2) DevTools → Application (Chrome) or Storage
            (Firefox) → Cookies → https://f95zone.to. 3) Copy{" "}
            <code className="text-[var(--fg)]">xf_user</code> and{" "}
            <code className="text-[var(--fg)]">xf_session</code> (
            <code className="text-[var(--fg)]">xf_csrf</code> helps). Paste as a Cookie header
            string.
          </p>
          <label className="block text-sm">
            <span className="field-label">Cookie header</span>
            <textarea
              className="input min-h-20"
              placeholder="xf_user=…; xf_session=…; xf_csrf=…"
              value={cookies}
              onChange={(e) => setCookies(e.target.value)}
            />
          </label>
          <button className="btn self-start" type="submit" disabled={busy || !cookies.trim()}>
            Save cookies
          </button>
        </form>
      </section>

      <section className="card card-section stack">
        <div>
          <h2 className="m-0 text-base font-semibold">Storage</h2>
          <p className="muted mt-1 text-sm">Usage under the data volume.</p>
        </div>
        {storage && (
          <dl className="m-0 grid gap-2 text-sm sm:grid-cols-2">
            <div className="file-row flex-col !items-start sm:col-span-2">
              <dt className="field-label mb-0">Data dir</dt>
              <dd className="m-0 break-all font-mono text-xs">{storage.data_dir}</dd>
            </div>
            <div className="file-row justify-between">
              <dt className="muted m-0">Database</dt>
              <dd className="m-0">{formatBytes(storage.database_bytes)}</dd>
            </div>
            <div className="file-row justify-between">
              <dt className="muted m-0">Media cache</dt>
              <dd className="m-0">{formatBytes(storage.media_cache_bytes)}</dd>
            </div>
            <div className="file-row justify-between">
              <dt className="muted m-0">Saves</dt>
              <dd className="m-0">{formatBytes(storage.saves_bytes)}</dd>
            </div>
            <div className="file-row justify-between">
              <dt className="muted m-0">Patches</dt>
              <dd className="m-0">{formatBytes(storage.patches_bytes)}</dd>
            </div>
            <div className="file-row justify-between sm:col-span-2">
              <dt className="m-0 font-medium">Total</dt>
              <dd className="m-0 font-medium">{formatBytes(storage.data_dir_bytes)}</dd>
            </div>
          </dl>
        )}
        <button
          type="button"
          className="btn btn-danger self-start"
          onClick={() =>
            void api
              .purgeMedia()
              .then(() => {
                toast.success("Media cache purged");
                return load();
              })
              .catch((err) => toast.error(err instanceof Error ? err.message : "Purge failed"))
          }
        >
          Purge media cache
        </button>
      </section>
    </div>
  );
}
