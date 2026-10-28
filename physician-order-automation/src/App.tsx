import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import AuthPanel from "./features/auth/AuthPanel";
import PatientRecordsPanel from "./features/patients/PatientRecordsPanel";

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

export default function App() {
  const [database, setDatabase] = useState<DatabaseStatus | null>(null);
  const [user, setUser] = useState<AuthenticatedUser | null>(null);
  const [error, setError] = useState("");

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
      <AuthPanel onAuthenticatedChange={setUser} />
      {user && <PatientRecordsPanel isAdmin={user.role === "admin"} />}
    </main>
  );
}
