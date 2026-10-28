import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Diagnosis } from "./DiagnosisEditor";
import type { PatientSelection } from "../patients/PatientRecordsPanel";

type Physician = { id: string; name: string; npi: string; active: boolean; sortOrder: number };
type OrderType = { id: string; name: string; active: boolean; sortOrder: number };
type SavePatientResult = { status: "created" | "updated" | "conflict"; patientId: string };
type ConfirmMode = "edited" | "conflict" | null;

function localDate() {
  const now = new Date();
  return new Date(now.getTime() - now.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
}

function samePatient(left: PatientSelection["draft"], right: PatientSelection["draft"]) {
  const keys: (keyof PatientSelection["draft"])[] = [
    "fullName", "gender", "address", "dob", "phone", "medicare", "otherInsurance",
  ];
  return keys.every((key) => (left[key] ?? "").trim().toLocaleLowerCase() === (right[key] ?? "").trim().toLocaleLowerCase());
}

export default function OrderDetailsPanel({ patient, diagnoses }: { patient: PatientSelection; diagnoses: Diagnosis[] }) {
  const [physicians, setPhysicians] = useState<Physician[]>([]);
  const [orderTypes, setOrderTypes] = useState<OrderType[]>([]);
  const [physicianId, setPhysicianId] = useState("");
  const [orderTypeId, setOrderTypeId] = useState("");
  const [orderDate, setOrderDate] = useState(localDate);
  const [comments, setComments] = useState("");
  const [confirmMode, setConfirmMode] = useState<ConfirmMode>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  const loadReferenceData = useCallback(async () => {
    const [physicianRows, typeRows] = await Promise.all([
      invoke<Physician[]>("list_physicians"),
      invoke<OrderType[]>("list_order_types"),
    ]);
    const activePhysicians = physicianRows.filter((entry) => entry.active);
    const activeOrderTypes = typeRows.filter((entry) => entry.active);
    setPhysicians(activePhysicians);
    setOrderTypes(activeOrderTypes);
    setPhysicianId((current) => current || activePhysicians[0]?.id || "");
    setOrderTypeId((current) => current || activeOrderTypes[0]?.id || "");
  }, []);

  useEffect(() => {
    loadReferenceData().catch((reason: unknown) => setMessage(String(reason)));
  }, [loadReferenceData]);

  async function exportPdf() {
    if (!patient.draft.fullName.trim()) {
      setMessage("Enter patient demographics before generating the order.");
      return;
    }
    if (!physicianId || !orderTypeId) {
      setMessage("Add an active physician and order type in Administration first.");
      return;
    }
    if (!orderDate) {
      setMessage("Select an order date.");
      return;
    }

    setBusy(true);
    setMessage("");
    try {
      const fileName = await invoke<string | null>("export_order", {
        request: { patient: patient.draft, physicianId, orderTypeId, orderDate, diagnoses, comments },
      });
      setMessage(fileName ? `Order saved as ${fileName}` : "Save canceled.");
      if (fileName) {
        setConfirmMode(null);
        setComments("");
        setOrderDate(localDate());
      }
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function savePatientThenExport() {
    setBusy(true);
    setMessage("");
    try {
      const saved = await invoke<SavePatientResult>("save_patient", { patient: patient.draft });
      if (saved.status === "conflict") {
        setConfirmMode("conflict");
        setMessage("A matching Medicare ID belongs to a record with different name or date-of-birth details.");
        return;
      }
      await exportPdf();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function generate() {
    if (patient.persistedId && patient.original && !samePatient(patient.draft, patient.original)) {
      setConfirmMode("edited");
      return;
    }
    if (!patient.persistedId) {
      await savePatientThenExport();
      return;
    }
    await exportPdf();
  }

  async function updatePatientAndGenerate() {
    setBusy(true);
    setMessage("");
    try {
      const saved = await invoke<SavePatientResult>("save_patient", { patient: patient.draft });
      if (saved.status === "conflict") {
        setConfirmMode("conflict");
        setMessage("The patient could not be updated because a matching Medicare ID has conflicting demographics.");
        return;
      }
      await exportPdf();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="rounded-lg border border-slate-200 bg-white p-6 shadow-sm">
      <h2 className="text-lg font-medium">Order details</h2>
      <p className="mt-1 text-sm text-slate-600">Choose the ordering physician and template, then add order-specific comments.</p>

      {message && <p role="status" className="mt-4 rounded bg-slate-50 p-3 text-sm text-slate-700">{message}</p>}

      <div className="mt-4 grid gap-4 sm:grid-cols-2">
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">Ordering physician
          <select className="rounded-md border border-slate-300 px-3 py-2" value={physicianId} onChange={(event) => setPhysicianId(event.target.value)}>
            <option value="">Select physician</option>
            {physicians.map((physician) => <option key={physician.id} value={physician.id}>{physician.name} · NPI {physician.npi}</option>)}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">Order type
          <select className="rounded-md border border-slate-300 px-3 py-2" value={orderTypeId} onChange={(event) => setOrderTypeId(event.target.value)}>
            <option value="">Select order type</option>
            {orderTypes.map((type) => <option key={type.id} value={type.id}>{type.name}</option>)}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">Order date
          <input className="rounded-md border border-slate-300 px-3 py-2" type="date" value={orderDate} onChange={(event) => setOrderDate(event.target.value)} />
        </label>
      </div>

      <label className="mt-4 flex flex-col gap-1 text-sm font-medium text-slate-700">Comments
        <textarea className="min-h-24 rounded-md border border-slate-300 px-3 py-2 text-sm" maxLength={5000} value={comments} onChange={(event) => setComments(event.target.value)} placeholder="Comments for this order" />
      </label>

      <div className="mt-4 flex flex-wrap items-center gap-2">
        <button className="rounded-md bg-blue-700 px-4 py-2 text-sm font-medium text-white hover:bg-blue-800 disabled:opacity-50" disabled={busy} onClick={() => void generate()} type="button">
          {busy ? "Generating…" : "Generate order PDF"}
        </button>
        <span className="text-xs text-slate-500">{diagnoses.length} diagnosis{diagnoses.length === 1 ? "" : "es"} included</span>
      </div>

      {confirmMode && (
        <div className="mt-4 rounded-md border border-amber-300 bg-amber-50 p-4" role="group" aria-label="Patient save choice">
          <p className="text-sm font-medium text-amber-900">
            {confirmMode === "edited"
              ? "The loaded patient's demographics have changed."
              : "The patient record has a matching identifier but conflicting demographics."}
          </p>
          <p className="mt-1 text-sm text-amber-800">Choose whether to update the saved record or use these demographics for this order only.</p>
          <div className="mt-3 flex flex-wrap gap-2">
            <button className="rounded-md bg-amber-800 px-3 py-2 text-sm text-white disabled:opacity-50" disabled={busy} onClick={() => void updatePatientAndGenerate()} type="button">Update record and generate</button>
            <button className="rounded-md border border-amber-500 px-3 py-2 text-sm text-amber-900 disabled:opacity-50" disabled={busy} onClick={() => { setConfirmMode(null); void exportPdf(); }} type="button">Use for this order only</button>
            <button className="px-3 py-2 text-sm text-slate-700" disabled={busy} onClick={() => setConfirmMode(null)} type="button">Cancel</button>
          </div>
        </div>
      )}
    </section>
  );
}
