import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import AuthPanel from "./features/auth/AuthPanel";
import AdminPage from "./features/admin/AdminPage";
import DailyOperationsPage from "./features/orders/DailyOperationsPage";

type DatabaseStatus = {
  schemaVersion: number;
  cipherVersion: string;
};

type AuthenticatedUser = {
  id: string;
  username: string;
  displayName: string;
  role: string;
  mustChangePassword: boolean;
};

type AppPage = "operations" | "admin";

export default function App() {
  const [database, setDatabase] = useState<DatabaseStatus | null>(null);
  const [user, setUser] = useState<AuthenticatedUser | null>(null);
  const [page, setPage] = useState<AppPage>("operations");
  const [error, setError] = useState("");

  const handleAuthenticatedChange = useCallback((nextUser: AuthenticatedUser | null) => {
    setUser(nextUser);
    if (!nextUser) setPage("operations");
  }, []);

  useEffect(() => {
    invoke<DatabaseStatus>("database_status")
      .then(setDatabase)
      .catch((reason: unknown) => setError(String(reason)));
  }, []);

  return (
    <main className="mx-auto flex min-h-screen max-w-xl flex-col gap-6 p-8 text-slate-900">
      <header>
        <h1 className="text-2xl font-semibold">Physician Order Automation</h1>
        <p className="mt-2 text-sm text-slate-600">
          {database
            ? `Local database ready · schema v${database.schemaVersion} · SQLCipher ${database.cipherVersion}`
            : "Opening local encrypted database…"}
        </p>
      </header>
      {error && <p role="alert" className="rounded bg-red-50 p-3 text-sm text-red-800">{error}</p>}
      <AuthPanel onAuthenticatedChange={handleAuthenticatedChange} />
      {user && (
        <>
          <nav className="flex gap-2 border-b border-slate-200" aria-label="Main navigation">
            <button
              aria-current={page === "operations" ? "page" : undefined}
              className={`border-b-2 px-3 py-2 text-sm font-medium ${page === "operations" ? "border-blue-700 text-blue-800" : "border-transparent text-slate-600 hover:text-slate-900"}`}
              onClick={() => setPage("operations")}
              type="button"
            >
              Daily operations
            </button>
            {user.role === "admin" && (
              <button
                aria-current={page === "admin" ? "page" : undefined}
                className={`border-b-2 px-3 py-2 text-sm font-medium ${page === "admin" ? "border-blue-700 text-blue-800" : "border-transparent text-slate-600 hover:text-slate-900"}`}
                onClick={() => setPage("admin")}
                type="button"
              >
                Administration
              </button>
            )}
          </nav>
          {page === "admin" && user.role === "admin" ? (
            <AdminPage />
          ) : (
            <DailyOperationsPage isAdmin={user.role === "admin"} />
          )}
        </>
      )}
    </main>
  );
}
