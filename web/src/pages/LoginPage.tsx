import { FormEvent, useState } from "react";
import { Navigate } from "react-router-dom";
import { useAuth } from "@/context/AuthContext";
import logo from "@/assets/avn-hub-logo.webp";

export function LoginPage() {
  const { loading, configured, authenticated, login } = useAuth();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (loading) {
    return <div className="muted p-8 text-center">Loading…</div>;
  }

  if (!configured || authenticated) {
    return <Navigate to="/" replace />;
  }

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await login(password);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex min-h-[70vh] items-center justify-center px-4 py-8">
      <form
        onSubmit={(e) => void onSubmit(e)}
        className="card w-full max-w-md space-y-5 p-6 sm:p-8"
      >
        <div className="flex flex-col items-center gap-3 text-center">
          <img src={logo} alt="AVN Hub" className="h-20 w-20 rounded-2xl object-cover" />
          <h1 className="page-title">AVN Hub</h1>
          <p className="muted m-0 text-sm">Sign in to your library</p>
        </div>
        <label className="block text-sm">
          <span className="field-label">Password</span>
          <input
            className="input"
            type="password"
            autoFocus
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        {error && <p className="m-0 text-sm text-[var(--danger)]">{error}</p>}
        <button className="btn btn-primary w-full" type="submit" disabled={busy}>
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </form>
    </div>
  );
}
