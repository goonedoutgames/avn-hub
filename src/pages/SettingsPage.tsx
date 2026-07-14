import { useEffect, useState } from "react";
import { HardDrive, KeyRound, Lock, LogOut, Save, Trash2 } from "lucide-react";
import { api, isWebMode } from "@/lib/api";
import { useAuth } from "@/context/AuthContext";
import { useTasks } from "@/context/TaskContext";
import type { Settings, StorageStats } from "@/lib/types";
import { formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { LibraryMigrationCard } from "@/components/LibraryMigrationCard";

export function SettingsPage() {
  const { runTask } = useTasks();
  const { refresh: refreshAuth } = useAuth();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [archivePath, setArchivePath] = useState("");
  const [f95Username, setF95Username] = useState("");
  const [f95Password, setF95Password] = useState("");
  const [f95Cookies, setF95Cookies] = useState("");
  const [httpAuthUsername, setHttpAuthUsername] = useState("");
  const [httpAuthPassword, setHttpAuthPassword] = useState("");
  const [saving, setSaving] = useState(false);
  const [loggingIn, setLoggingIn] = useState(false);
  const [purging, setPurging] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [storage, setStorage] = useState<StorageStats | null>(null);

  useEffect(() => {
    api.getSettings().then((s) => {
      setSettings(s);
      setArchivePath(s.archive_path);
      setF95Username(s.f95_username ?? "");
      setF95Cookies(s.f95_cookies ?? "");
      setHttpAuthUsername(s.http_auth_username ?? "");
    });
    api.getStorageStats().then(setStorage).catch(() => setStorage(null));
  }, []);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setMessage(null);
    try {
      const updated = await runTask(
        "save-settings",
        "Saving settings",
        async (update) => {
          update("Writing configuration…");
          return api.updateSettings({
            archive_path: archivePath,
            f95_username: f95Username || undefined,
            f95_password: f95Password || undefined,
            f95_cookies: f95Cookies || undefined,
            http_auth_username: httpAuthUsername || undefined,
            http_auth_password: httpAuthPassword || undefined,
          });
        },
      );
      setSettings(updated);
      setF95Password("");
      setHttpAuthPassword("");
      if (isWebMode() && httpAuthPassword && httpAuthUsername) {
        try {
          await api.login(httpAuthUsername, httpAuthPassword);
        } catch {
          // user can sign in manually from the login page
        }
      }
      await refreshAuth();
      setMessage(
        updated.f95_authenticated
          ? "Settings saved and F95Zone session active"
          : "Settings saved",
      );
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Failed to save");
    } finally {
      setSaving(false);
    }
  };

  const handleLogin = async () => {
    setLoggingIn(true);
    setMessage(null);
    try {
      if (f95Username || f95Password) {
        await runTask("save-creds", "Saving credentials", async () => {
          await api.updateSettings({
            f95_username: f95Username || undefined,
            f95_password: f95Password || undefined,
          });
        });
      }
      const result = await runTask(
        "f95-login",
        "Logging in to F95Zone",
        async (update) => {
          update("Authenticating with f95zone.to…", 30);
          return api.f95Login({
            username: f95Username || undefined,
            password: f95Password || undefined,
          });
        },
      );
      setSettings(await api.getSettings());
      setF95Password("");
      setMessage(result.message);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Login failed");
    } finally {
      setLoggingIn(false);
    }
  };

  const handlePurgeMedia = async () => {
    if (
      !window.confirm(
        "Delete all cached cover images and screenshots? Matched games keep their metadata; re-match to re-download media.",
      )
    ) {
      return;
    }
    setPurging(true);
    setMessage(null);
    try {
      await runTask("purge-media", "Purging cached media", async () => {
        await api.purgeMediaCache();
      });
      setMessage("Cached media removed. Re-match games to download fresh images.");
      api.getStorageStats().then(setStorage).catch(() => undefined);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : "Failed to purge media");
    } finally {
      setPurging(false);
    }
  };

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Settings</h1>
        <p className="text-sm text-[var(--color-muted-foreground)]">
          Configure archive folder and F95Zone authentication
        </p>
      </div>

      <LibraryMigrationCard />

      <form onSubmit={handleSave} className="space-y-6">
        {storage && (
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <HardDrive className="h-4 w-4" />
                Storage
              </CardTitle>
              <CardDescription>
                Disk usage for your archives and app data
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <StorageRow
                label="Game archives"
                used={storage.archives_bytes}
                total={storage.archive_volume_total}
                available={storage.archive_volume_available}
                detail={storage.archive_path || "Not configured"}
              />
              <StorageRow
                label="Cached media"
                used={storage.media_cache_bytes}
                total={storage.data_volume_total}
                available={storage.data_volume_available}
                detail={`${formatBytes(storage.database_bytes)} database · ${formatBytes(storage.data_dir_bytes)} total in data dir`}
              />
            </CardContent>
          </Card>
        )}

        <Card>
          <CardHeader>
            <CardTitle>Archive Folder</CardTitle>
            <CardDescription>
              Path to the folder containing game archives (.zip, .rar, .7z,
              .bz2). In Docker, mount this as a volume.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Input
              value={archivePath}
              onChange={(e) => setArchivePath(e.target.value)}
              placeholder="/path/to/archives"
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>F95Zone Account</CardTitle>
            <CardDescription>
              Enter your F95Zone credentials. The app logs in via the official
              login endpoint and caches session cookies locally.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input
              value={f95Username}
              onChange={(e) => setF95Username(e.target.value)}
              placeholder="Username"
              autoComplete="username"
            />
            <Input
              type="password"
              value={f95Password}
              onChange={(e) => setF95Password(e.target.value)}
              placeholder={
                settings?.f95_password_set
                  ? "Password (saved — leave blank to keep)"
                  : "Password"
              }
              autoComplete="current-password"
            />
            <div className="flex items-center gap-3">
              <Button
                type="button"
                variant="secondary"
                onClick={handleLogin}
                disabled={loggingIn}
              >
                <KeyRound className="h-4 w-4" />
                {loggingIn ? "Logging in..." : "Login to F95Zone"}
              </Button>
              {settings?.f95_authenticated && (
                <span className="text-xs text-green-400">Session active</span>
              )}
            </div>
          </CardContent>
        </Card>

        {isWebMode() && (
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Lock className="h-4 w-4" />
                Web Login
              </CardTitle>
              <CardDescription>
                Protect the web UI with a username and password. Sessions last
                about 7 days. Required for public servers.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <Input
                value={httpAuthUsername}
                onChange={(e) => setHttpAuthUsername(e.target.value)}
                placeholder="Username"
                autoComplete="username"
              />
              <Input
                type="password"
                value={httpAuthPassword}
                onChange={(e) => setHttpAuthPassword(e.target.value)}
                placeholder={
                  settings?.http_auth_configured
                    ? "New password (leave blank to keep)"
                    : "Password"
                }
                autoComplete="new-password"
              />
              {settings?.http_auth_configured && (
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-xs text-green-400">
                    Login enabled
                    {settings.http_auth_username
                      ? ` for “${settings.http_auth_username}”`
                      : ""}
                  </span>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={async () => {
                      if (
                        !window.confirm(
                          "Remove web login? The server will be open to anyone with the URL until you set credentials again.",
                        )
                      ) {
                        return;
                      }
                      const updated = await api.updateSettings({
                        http_auth_remove: true,
                      });
                      setSettings(updated);
                      setHttpAuthUsername("");
                      setHttpAuthPassword("");
                      await refreshAuth();
                      setMessage("Web login removed");
                    }}
                  >
                    <LogOut className="h-3 w-3" />
                    Remove login
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        )}

        <Card>
          <CardHeader>
            <CardTitle>Cookie Fallback</CardTitle>
            <CardDescription>
              Optional: paste browser cookies if credential login fails (2FA,
              CAPTCHA). From DevTools → Application → Cookies → f95zone.to
            </CardDescription>
          </CardHeader>
          <CardContent>
            <textarea
              value={f95Cookies}
              onChange={(e) => setF95Cookies(e.target.value)}
              placeholder="xf_session=...; xf_user=..."
              rows={3}
              className="flex w-full rounded-md border border-[var(--color-input)] bg-[var(--color-background)] px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-ring)]"
            />
          </CardContent>
        </Card>

        {settings && (
          <Card>
            <CardHeader>
              <CardTitle>Data Directory</CardTitle>
              <CardDescription>
                SQLite database and cached media are stored here.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <code className="text-sm text-[var(--color-muted-foreground)]">
                {settings.data_dir}
              </code>
              <div>
                <Button
                  type="button"
                  variant="destructive"
                  onClick={handlePurgeMedia}
                  disabled={purging}
                >
                  <Trash2 className="h-4 w-4" />
                  {purging ? "Purging…" : "Purge cached media"}
                </Button>
                <p className="mt-2 text-xs text-[var(--color-muted-foreground)]">
                  Removes downloaded covers and screenshots. Use after fixing image
                  issues, then re-match affected games.
                </p>
              </div>
            </CardContent>
          </Card>
        )}

        {message && (
          <p className="text-sm text-[var(--color-muted-foreground)]">
            {message}
          </p>
        )}

        <Button type="submit" disabled={saving}>
          <Save className="h-4 w-4" />
          {saving ? "Saving..." : "Save Settings"}
        </Button>
      </form>
    </div>
  );
}

function StorageRow({
  label,
  used,
  total,
  available,
  detail,
}: {
  label: string;
  used: number;
  total: number | null;
  available: number | null;
  detail: string;
}) {
  const usedNum = Number(used);
  const pct =
    total != null && total > 0
      ? Math.min(100, (usedNum / total) * 100)
      : null;

  return (
    <div className="space-y-2">
      <div className="flex items-baseline justify-between gap-2 text-sm">
        <span className="font-medium">{label}</span>
        <span className="text-[var(--color-muted-foreground)]">
          {formatBytes(usedNum)}
          {total != null && ` / ${formatBytes(total)}`}
        </span>
      </div>
      {pct != null && (
        <div className="h-2 overflow-hidden rounded-full bg-[var(--color-muted)]">
          <div
            className="h-full rounded-full bg-[var(--color-primary)] transition-all"
            style={{ width: `${pct}%` }}
          />
        </div>
      )}
      <p className="truncate text-xs text-[var(--color-muted-foreground)]">
        {detail}
        {available != null && ` · ${formatBytes(available)} free`}
      </p>
    </div>
  );
}
