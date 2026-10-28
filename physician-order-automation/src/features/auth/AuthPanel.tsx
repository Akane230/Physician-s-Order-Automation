import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

type AuthenticatedUser = {
  id: string;
  username: string;
  displayName: string;
  role: string;
  mustChangePassword: boolean;
};

type AuthStatus = {
  setupRequired: boolean;
  user: AuthenticatedUser | null;
};

export default function AuthPanel() {
  const [auth, setAuth] = useState<AuthStatus | null>(null);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setAuth(await invoke<AuthStatus>("auth_status"));
  }, []);

  useEffect(() => {
    refresh().catch((reason: unknown) => setError(String(reason)));
  }, [refresh]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      if (auth?.setupRequired) {
        await invoke("setup_initial_admin", { username, displayName, password });
      } else {
        await invoke("login", { username, password });
      }
      setPassword("");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function signOut() {
    setBusy(true);
    setError("");
    try {
      await invoke("logout");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="rounded-lg border border-slate-200 p-6">
      {error && <p role="alert" className="mb-4 rounded bg-red-50 p-3 text-sm text-red-800">{error}</p>}
      {!auth ? (
        <p>Checking local account status…</p>
      ) : auth.user ? (
        <>
          <h2 className="text-lg font-medium">Signed in</h2>
          <p className="mt-2">{auth.user.displayName} · {auth.user.role}</p>
          <button className="mt-5 rounded bg-slate-800 px-4 py-2 text-white disabled:opacity-50" disabled={busy} onClick={signOut}>
            Sign out
          </button>
        </>
      ) : (
        <>
          <h2 className="text-lg font-medium">
            {auth.setupRequired ? "Create the first administrator" : "Sign in"}
          </h2>
          <form className="mt-5 flex flex-col gap-4" onSubmit={submit}>
            <label className="flex flex-col gap-1 text-sm">
              Username
              <input className="rounded border border-slate-300 px-3 py-2" autoComplete="username" required maxLength={64} value={username} onChange={(event) => setUsername(event.target.value)} />
            </label>
            {auth.setupRequired && (
              <label className="flex flex-col gap-1 text-sm">
                Display name
                <input className="rounded border border-slate-300 px-3 py-2" autoComplete="name" required maxLength={120} value={displayName} onChange={(event) => setDisplayName(event.target.value)} />
              </label>
            )}
            <label className="flex flex-col gap-1 text-sm">
              Password
              <input className="rounded border border-slate-300 px-3 py-2" type="password" autoComplete={auth.setupRequired ? "new-password" : "current-password"} required minLength={auth.setupRequired ? 12 : undefined} maxLength={256} value={password} onChange={(event) => setPassword(event.target.value)} />
              {auth.setupRequired && <span className="text-xs text-slate-500">Use at least 12 characters.</span>}
            </label>
            <button className="self-start rounded bg-blue-700 px-4 py-2 text-white disabled:opacity-50" disabled={busy}>
              {busy ? "Please wait…" : auth.setupRequired ? "Create administrator" : "Sign in"}
            </button>
          </form>
        </>
      )}
    </section>
  );
}
