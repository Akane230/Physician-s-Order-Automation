import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { FormEvent } from "react";

type PatientRecord = {
  id: string;
  fullName: string;
  gender: string | null;
  address: string | null;
  dob: string | null;
  phone: string | null;
  medicare: string | null;
  otherInsurance: string | null;
  createdAt: string;
  updatedAt: string;
};

export type PatientDraft = Omit<PatientRecord, "id" | "createdAt" | "updatedAt">;
export type PatientSelection = {
  draft: PatientDraft;
  persistedId: string | null;
  original: PatientDraft | null;
};
type SaveResult = { status: "created" | "updated" | "conflict"; patientId: string };
type ParsedDemographics = Partial<PatientDraft> & { warnings: string[] };

const emptyPatient: PatientDraft = {
  fullName: "",
  gender: "",
  address: "",
  dob: "",
  phone: "",
  medicare: "",
  otherInsurance: "",
};

export default function PatientRecordsPanel({ isAdmin, onPatientChange }: {
  isAdmin: boolean;
  onPatientChange: (patient: PatientSelection) => void;
}) {
  const [patient, setPatient] = useState<PatientDraft>({ ...emptyPatient });
  const [persistedId, setPersistedId] = useState<string | null>(null);
  const [original, setOriginal] = useState<PatientDraft | null>(null);
  const [records, setRecords] = useState<PatientRecord[]>([]);
  const [query, setQuery] = useState("");
  const [pasteText, setPasteText] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    onPatientChange({ draft: patient, persistedId, original });
  }, [onPatientChange, patient, persistedId, original]);

  function update<K extends keyof PatientDraft>(key: K, value: PatientDraft[K]) {
    setPatient((current) => ({ ...current, [key]: value }));
  }

  async function search(preserveMessage = false) {
    setBusy(true);
    if (!preserveMessage) setMessage("");
    try {
      setRecords(await invoke<PatientRecord[]>("search_patients", { query }));
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function autoFill() {
    setBusy(true);
    setMessage("");
    try {
      const parsed = await invoke<ParsedDemographics>("parse_demographics", { text: pasteText });
      setPatient((current) => ({
        ...current,
        fullName: parsed.fullName ?? current.fullName,
        gender: parsed.gender ?? current.gender,
        address: parsed.address ?? current.address,
        dob: parsed.dob ?? current.dob,
        phone: parsed.phone ?? current.phone,
        medicare: parsed.medicare ?? current.medicare,
        otherInsurance: parsed.otherInsurance ?? current.otherInsurance,
      }));
      setMessage(parsed.warnings.join(" ") || "Review the populated fields before saving.");
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    try {
      const result = await invoke<SaveResult>("save_patient", { patient });
      if (result.status === "conflict") {
        setMessage("A possible duplicate has different demographic details. Review the matching record before saving.");
      } else {
        setPersistedId(result.patientId);
        setOriginal({ ...patient });
        setMessage(result.status === "created" ? "Patient record saved." : "Patient record updated.");
        await search(true);
      }
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function removePatient(record: PatientRecord) {
    if (!window.confirm(`Delete the saved record for ${record.fullName}?`)) return;
    setBusy(true);
    setMessage("");
    try {
      const deleted = await invoke<boolean>("delete_patient", { patientId: record.id });
      setMessage(deleted ? "Patient record deleted." : "Patient record was not found.");
      await search(true);
      if (persistedId === record.id) {
        setPatient({ ...emptyPatient });
        setPersistedId(null);
        setOriginal(null);
      }
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function selectRecord(record: PatientRecord) {
    const selected = {
      fullName: record.fullName,
      gender: record.gender ?? "",
      address: record.address ?? "",
      dob: record.dob ?? "",
      phone: record.phone ?? "",
      medicare: record.medicare ?? "",
      otherInsurance: record.otherInsurance ?? "",
    };
    setPatient(selected);
    setPersistedId(record.id);
    setOriginal(selected);
    setMessage("Loaded patient demographics into the form.");
  }

  function startNewPatient() {
    setPatient({ ...emptyPatient });
    setPersistedId(null);
    setOriginal(null);
    setMessage("");
  }

  return (
    <section className="rounded-lg border border-slate-200 p-6">
      <h2 className="text-lg font-medium">Patient records</h2>
      <p className="mt-1 text-sm text-slate-600">Only patient demographics are saved. Diagnoses and order details are not part of this record.</p>
      {message && <p role="status" className="mt-3 rounded bg-slate-50 p-3 text-sm">{message}</p>}

      <div className="mt-5">
        <label className="flex flex-col gap-1 text-sm" htmlFor="demographicsPaste">Paste patient demographics</label>
        <textarea id="demographicsPaste" className="mt-2 min-h-28 w-full rounded border border-slate-300 px-3 py-2" placeholder="Paste labeled demographics from an intake sheet. The text is parsed locally and is not saved." value={pasteText} onChange={(event) => setPasteText(event.target.value)} />
        <div className="mt-2 flex gap-2">
          <button className="rounded bg-slate-800 px-3 py-2 text-white disabled:opacity-50" disabled={busy || !pasteText.trim()} onClick={() => void autoFill()}>Auto-fill fields</button>
          <button className="rounded border border-slate-300 px-3 py-2" onClick={() => setPasteText("")}>Clear paste</button>
        </div>
      </div>

      <button className="mt-3 text-sm text-blue-700 hover:underline" type="button" onClick={startNewPatient}>
        Start a new patient
      </button>

      <div className="mt-5 flex gap-2">
        <input className="min-w-0 flex-1 rounded border border-slate-300 px-3 py-2" aria-label="Search patient records" placeholder="Search name, phone, or Medicare" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void search(); }} />
        <button className="rounded border border-slate-300 px-3 py-2 disabled:opacity-50" disabled={busy} onClick={() => void search()}>Search</button>
      </div>

      {records.length > 0 && (
        <ul className="mt-3 divide-y divide-slate-200 rounded border border-slate-200">
          {records.map((record) => (
            <li key={record.id} className="flex items-center justify-between gap-3 p-3">
              <button className="min-w-0 text-left" onClick={() => selectRecord(record)}>
                <span className="block truncate font-medium">{record.fullName}</span>
                <span className="text-sm text-slate-600">{record.dob ?? "No DOB"} · {record.phone ?? "No phone"}</span>
              </button>
              {isAdmin && <button className="text-sm text-red-700" disabled={busy} onClick={() => void removePatient(record)}>Delete</button>}
            </li>
          ))}
        </ul>
      )}

      <form className="mt-6 grid gap-4 sm:grid-cols-2" onSubmit={save}>
        <label className="flex flex-col gap-1 text-sm sm:col-span-2">Patient name
          <input className="rounded border border-slate-300 px-3 py-2" required maxLength={200} value={patient.fullName} onChange={(event) => update("fullName", event.target.value)} />
        </label>
        <label className="flex flex-col gap-1 text-sm">Gender
          <select className="rounded border border-slate-300 px-3 py-2" value={patient.gender ?? ""} onChange={(event) => update("gender", event.target.value)}>
            <option value="">Select</option><option>Female</option><option>Male</option><option>Other</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-sm">Date of birth
          <input className="rounded border border-slate-300 px-3 py-2" type="date" value={patient.dob ?? ""} onChange={(event) => update("dob", event.target.value)} />
        </label>
        <label className="flex flex-col gap-1 text-sm sm:col-span-2">Address
          <input className="rounded border border-slate-300 px-3 py-2" maxLength={500} value={patient.address ?? ""} onChange={(event) => update("address", event.target.value)} />
        </label>
        <label className="flex flex-col gap-1 text-sm">Phone
          <input className="rounded border border-slate-300 px-3 py-2" maxLength={64} value={patient.phone ?? ""} onChange={(event) => update("phone", event.target.value)} />
        </label>
        <label className="flex flex-col gap-1 text-sm">Medicare
          <input className="rounded border border-slate-300 px-3 py-2" maxLength={64} value={patient.medicare ?? ""} onChange={(event) => update("medicare", event.target.value)} />
        </label>
        <label className="flex flex-col gap-1 text-sm sm:col-span-2">Other insurance
          <input className="rounded border border-slate-300 px-3 py-2" maxLength={200} value={patient.otherInsurance ?? ""} onChange={(event) => update("otherInsurance", event.target.value)} />
        </label>
        <button className="self-start rounded bg-blue-700 px-4 py-2 text-white disabled:opacity-50" disabled={busy}>Save patient record</button>
      </form>
    </section>
  );
}
