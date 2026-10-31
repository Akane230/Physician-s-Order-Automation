import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
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

export type PatientDraft = Omit<
  PatientRecord,
  "id" | "createdAt" | "updatedAt"
>;
export type PatientSelection = {
  draft: PatientDraft;
  persistedId: string | null;
  original: PatientDraft | null;
};
type SaveResult = {
  status: "created" | "updated" | "conflict";
  patientId: string;
};
type ParsedDemographics = Partial<PatientDraft> & { warnings: string[] };
const importFields: (keyof PatientDraft)[] = [
  "fullName",
  "gender",
  "address",
  "dob",
  "phone",
  "medicare",
  "otherInsurance",
];

const emptyPatient: PatientDraft = {
  fullName: "",
  gender: "",
  address: "",
  dob: "",
  phone: "",
  medicare: "",
  otherInsurance: "",
};

export default function PatientRecordsPanel({
  onPatientChange,
}: {
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
  const [dragOver, setDragOver] = useState(false);
  const demographicsRef = useRef<HTMLElement>(null);

  useEffect(() => {
    onPatientChange({ draft: patient, persistedId, original });
  }, [onPatientChange, patient, persistedId, original]);

  function update<K extends keyof PatientDraft>(
    key: K,
    value: PatientDraft[K],
  ) {
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
      const parsed = await invoke<ParsedDemographics>("parse_demographics", {
        text: pasteText,
      });
      applyImportedDemographics(parsed);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function applyImportedDemographics(parsed: ParsedDemographics) {
    const updates = Object.fromEntries(
      importFields
        .filter((field) => parsed[field]?.trim() && !patient[field]?.trim())
        .map((field) => [field, parsed[field]]),
    ) as Partial<PatientDraft>;
    setPatient((current) => ({ ...current, ...updates }));
  }

  async function importPdf() {
    setBusy(true);
    setMessage("");
    try {
      const parsed = await invoke<ParsedDemographics | null>(
        "import_demographics_pdf",
      );
      if (!parsed) return;
      applyImportedDemographics(parsed);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function importDroppedPdf(path: string) {
    if (!path.toLowerCase().endsWith(".pdf")) {
      setMessage(
        "That file is not a PDF. Drop a PDF document to import demographics.",
      );
      return;
    }
    setBusy(true);
    setMessage("");
    try {
      const parsed = await invoke<ParsedDemographics>(
        "import_demographics_pdf_path",
        { path },
      );
      applyImportedDemographics(parsed);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const target = demographicsRef.current;
        const point =
          "position" in event.payload ? event.payload.position : null;
        const bounds = target?.getBoundingClientRect();
        const inside = Boolean(
          point &&
          bounds &&
          point.x / window.devicePixelRatio >= bounds.left &&
          point.x / window.devicePixelRatio <= bounds.right &&
          point.y / window.devicePixelRatio >= bounds.top &&
          point.y / window.devicePixelRatio <= bounds.bottom,
        );
        if (event.payload.type === "enter" || event.payload.type === "over")
          setDragOver(inside);
        if (event.payload.type === "leave") setDragOver(false);
        if (event.payload.type === "drop") {
          setDragOver(false);
          if (inside && event.payload.paths.length === 1)
            void importDroppedPdf(event.payload.paths[0]);
          else if (inside) setMessage("Drop one PDF at a time.");
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() =>
        setMessage(
          "File drag and drop is unavailable. Use the PDF import button instead.",
        ),
      );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    try {
      const result = await invoke<SaveResult>("save_patient", { patient });
      if (result.status === "conflict") {
        setMessage(
          "A possible duplicate has different demographic details. Review the matching record before saving.",
        );
      } else {
        setPersistedId(result.patientId);
        setOriginal({ ...patient });
        setMessage(
          result.status === "created"
            ? "Patient record saved."
            : "Patient record updated.",
        );
        await search(true);
      }
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function removePatient(record: PatientRecord) {
    if (!window.confirm(`Delete the saved record for ${record.fullName}?`))
      return;
    setBusy(true);
    setMessage("");
    try {
      const deleted = await invoke<boolean>("delete_patient", {
        patientId: record.id,
      });
      setMessage(
        deleted ? "Patient record deleted." : "Patient record was not found.",
      );
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
    <section
      ref={demographicsRef}
      className={`relative rounded-xl border bg-white p-4 shadow-sm sm:p-6 ${dragOver ? "border-blue-600 ring-2 ring-blue-300" : "border-slate-200"}`}
    >
      <h2 className="flex items-center gap-3 text-lg font-semibold text-blue-950">
        1. Patient demographics
      </h2>
      <p className="mt-1 text-sm text-slate-600">
        Only patient demographics are saved. Diagnoses and order details are not
        part of this record.
      </p>
      {message && (
        <p role="status" className="mt-3 rounded bg-slate-50 p-3 text-sm">
          {message}
        </p>
      )}

      <div className="mt-5 rounded-lg bg-slate-50 p-4">
        <div className="mb-3">
          <button
            type="button"
            className="rounded border border-slate-300 bg-white px-3 py-2 disabled:opacity-50"
            disabled={busy}
            onClick={() => void importPdf()}
          >
            Import demographics from PDF
          </button>
          <p className="mt-1 text-xs text-slate-600">
            You can also drag a PDF onto this patient demographics section.
          </p>
        </div>
        <label
          className="flex flex-col gap-1 text-sm"
          htmlFor="demographicsPaste"
        >
          Paste patient demographics
        </label>
        <textarea
          id="demographicsPaste"
          className="mt-2 min-h-28 w-full rounded border border-slate-300 px-3 py-2"
          placeholder="Paste labeled demographics from an intake sheet. The text is parsed locally and is not saved."
          value={pasteText}
          onChange={(event) => setPasteText(event.target.value)}
        />
        <div className="mt-2 flex gap-2">
          <button
            className="rounded bg-slate-800 px-3 py-2 text-white disabled:opacity-50"
            disabled={busy || !pasteText.trim()}
            onClick={() => void autoFill()}
          >
            Auto-fill fields
          </button>
          <button
            className="rounded border border-slate-300 px-3 py-2"
            onClick={() => setPasteText("")}
          >
            Clear paste
          </button>
        </div>
      </div>

      <div className="mt-5 flex flex-wrap gap-2">
        <input
          className="min-w-0 flex-1 rounded border border-slate-300 px-3 py-2"
          aria-label="Search patient records"
          placeholder="Search name, phone, or primary insurance ID"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void search();
          }}
        />
        <button
          className="rounded border text-white bg-blue-700 border-slate-300 px-3 py-2 disabled:opacity-50"
          disabled={busy}
          onClick={() => void search()}
        >
          Search
        </button>
        <button
          className="rounded border border-slate-300 px-3 py-2 disabled:opacity-50"
          disabled={busy}
          onClick={startNewPatient}
        >
          Clear
        </button>
      </div>

      {records.length > 0 && (
        <ul className="mt-3 divide-y divide-slate-200 rounded border border-slate-200">
          {records.map((record) => (
            <li
              key={record.id}
              className="flex items-center justify-between gap-3 p-3"
            >
              <button
                className="min-w-0 text-left"
                onClick={() => selectRecord(record)}
              >
                <span className="block truncate font-medium">
                  {record.fullName}
                </span>
                <span className="text-sm text-slate-600">
                  {record.dob ?? "No DOB"} · {record.phone ?? "No phone"}
                </span>
              </button>
              <button
                className="text-sm text-red-700"
                disabled={busy}
                onClick={() => void removePatient(record)}
              >
                Delete
              </button>
            </li>
          ))}
        </ul>
      )}

      <form className="mt-6 grid gap-4 sm:grid-cols-2" onSubmit={save}>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700 sm:col-span-2">
          Patient name
          <input
            className="rounded border border-slate-300 px-3 py-2"
            required
            maxLength={200}
            value={patient.fullName}
            onChange={(event) => update("fullName", event.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">
          Gender
          <select
            className="rounded border border-slate-300 px-3 py-2"
            value={patient.gender ?? ""}
            onChange={(event) => update("gender", event.target.value)}
          >
            <option value="">Select</option>
            <option>Female</option>
            <option>Male</option>
            <option>Other</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">
          Date of birth
          <input
            className="rounded border border-slate-300 px-3 py-2"
            type="date"
            value={patient.dob ?? ""}
            onChange={(event) => update("dob", event.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700 sm:col-span-2">
          Address
          <input
            className="rounded border border-slate-300 px-3 py-2"
            maxLength={500}
            value={patient.address ?? ""}
            onChange={(event) => update("address", event.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">
          Phone
          <input
            className="rounded border border-slate-300 px-3 py-2"
            maxLength={64}
            value={patient.phone ?? ""}
            onChange={(event) => update("phone", event.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">
          Medicare
          <input
            className="rounded border border-slate-300 px-3 py-2"
            maxLength={64}
            value={patient.medicare ?? ""}
            onChange={(event) => update("medicare", event.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700 sm:col-span-2">
          Other Insurance
          <input
            className="rounded border border-slate-300 px-3 py-2"
            maxLength={200}
            value={patient.otherInsurance ?? ""}
            onChange={(event) => update("otherInsurance", event.target.value)}
          />
        </label>
        <button className="self-start rounded-md bg-blue-950 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-900 disabled:opacity-50">
          Save patient record
        </button>
      </form>
    </section>
  );
}
