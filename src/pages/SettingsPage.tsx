import { useEffect, useState } from "react";
import { KeyRound, Save, Trash2 } from "lucide-react";
import { api } from "@/lib/api";
import { useTasks } from "@/context/TaskContext";
import type { Settings } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export function SettingsPage() {
  const { runTask } = useTasks();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [archivePath, setArchivePath] = useState("");
  const [f95Username, setF95Username] = useState("");
  const [f95Password, setF95Password] = useState("");
  const [f95Cookies, setF95Cookies] = useState("");
  const [saving, setSaving] = useState(false);
  const [loggingIn, setLoggingIn] = useState(false);
  const [purging, setPurging] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    api.getSettings().then((s) => {
      setSettings(s);
      setArchivePath(s.archive_path);
      setF95Username(s.f95_username ?? "");
      setF95Cookies(s.f95_cookies ?? "");
    });
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
          });
        },
      );
      setSettings(updated);
      setF95Password("");
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

      <form onSubmit={handleSave} className="space-y-6">
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
