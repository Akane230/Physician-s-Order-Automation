import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { FormEvent } from "react";

export type Diagnosis = {
  code: string | null;
  description: string;
};

export default function DiagnosisEditor({ onDiagnosesChange }: { onDiagnosesChange: (diagnoses: Diagnosis[]) => void }) {
  const [sourceText, setSourceText] = useState("");
  const [diagnoses, setDiagnoses] = useState<Diagnosis[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    onDiagnosesChange(diagnoses);
  }, [diagnoses, onDiagnosesChange]);

  async function parse(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    try {
      const parsed = await invoke<Diagnosis[]>("parse_diagnoses", { text: sourceText });
      setDiagnoses(parsed);
      setMessage(parsed.length ? `Found ${parsed.length} diagnosis${parsed.length === 1 ? "" : "es"}. Review each entry.` : "No diagnoses found.");
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function clear() {
    setSourceText("");
    setDiagnoses([]);
    setMessage("");
  }

  return (
    <section className="rounded-lg border border-slate-200 bg-white p-6 shadow-sm">
      <div>
        <h2 className="text-lg font-medium">Diagnoses for this order</h2>
        <p className="mt-1 text-sm text-slate-600">
          Paste the diagnosis list, then review the parsed entries. This does not validate codes against the ICD catalog.
        </p>
      </div>

      <form className="mt-4" onSubmit={parse}>
        <label className="sr-only" htmlFor="diagnosisPaste">Paste diagnoses</label>
        <textarea
          id="diagnosisPaste"
          className="min-h-32 w-full rounded-md border border-slate-300 px-3 py-2 text-sm shadow-sm outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-100"
          placeholder={"Paste one diagnosis per line, for example:\nICD-10-CM Condition: G81.94 Hemiplegia, unspecified affecting left non-dominant side"}
          value={sourceText}
          onChange={(event) => setSourceText(event.target.value)}
        />
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            className="rounded-md bg-blue-700 px-4 py-2 text-sm font-medium text-white hover:bg-blue-800 disabled:cursor-not-allowed disabled:opacity-50"
            disabled={busy || !sourceText.trim()}
          >
            {busy ? "Parsing…" : "Parse diagnoses"}
          </button>
          <button className="rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50" type="button" onClick={clear}>
            Clear order diagnoses
          </button>
        </div>
      </form>

      {message && <p className="mt-3 text-sm text-slate-600" role="status">{message}</p>}

      {diagnoses.length > 0 && (
        <ul className="mt-4 divide-y divide-slate-200 rounded-md border border-slate-200">
          {diagnoses.map((diagnosis, index) => (
            <li className="flex items-start justify-between gap-3 p-3" key={`${diagnosis.code ?? "description"}-${index}`}>
              <div className="min-w-0">
                {diagnosis.code ? (
                  <span className="inline-flex rounded bg-blue-50 px-2 py-0.5 font-mono text-xs font-semibold text-blue-800">{diagnosis.code}</span>
                ) : (
                  <span className="text-xs font-medium text-amber-700">No code detected</span>
                )}
                <p className="mt-1 text-sm text-slate-800">{diagnosis.description || "No description provided"}</p>
              </div>
              <button
                aria-label={`Remove diagnosis ${diagnosis.code ?? index + 1}`}
                className="shrink-0 rounded px-2 py-1 text-sm text-slate-500 hover:bg-red-50 hover:text-red-700"
                onClick={() => setDiagnoses((current) => current.filter((_, itemIndex) => itemIndex !== index))}
                type="button"
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
