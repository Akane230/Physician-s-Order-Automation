import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function App() {
  const [msg, setMsg] = useState("Checking local database...");

  useEffect(() => {
    invoke<{ schemaVersion: number; cipherVersion: string }>("database_status")
      .then(({ schemaVersion, cipherVersion }) =>
        setMsg(`Database ready · schema v${schemaVersion} · SQLCipher ${cipherVersion}`),
      )
      .catch((e) => setMsg("ERROR: " + String(e)));
  }, []);

  return <h1 className="p-6 text-xl">{msg}</h1>;
}
