import { useState } from "react";
import DiagnosisEditor from "./DiagnosisEditor";
import type { Diagnosis } from "./DiagnosisEditor";
import OrderDetailsPanel from "./OrderDetailsPanel";
import PatientRecordsPanel from "../patients/PatientRecordsPanel";
import type { PatientSelection } from "../patients/PatientRecordsPanel";

const emptyPatient: PatientSelection = {
  draft: {
    fullName: "",
    gender: "",
    address: "",
    dob: "",
    phone: "",
    medicare: "",
    otherInsurance: "",
  },
  persistedId: null,
  original: null,
};

export default function DailyOperationsPage() {
  const [patient, setPatient] = useState<PatientSelection>(emptyPatient);
  const [diagnoses, setDiagnoses] = useState<Diagnosis[]>([]);
  const [formVersion, setFormVersion] = useState(0);

  function startNewOrder() {
    setPatient(emptyPatient);
    setDiagnoses([]);
    setFormVersion((version) => version + 1);
  }

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold text-blue-950">
            Daily operations
          </h2>
          <p className="mt-1 text-sm text-slate-600">
            Prepare a patient-specific physician order.
          </p>
        </div>
        <button
          className="rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
          onClick={startNewOrder}
          type="button"
        >
          Start new order
        </button>
      </header>
      <PatientRecordsPanel
        key={`patient-${formVersion}`}
        onPatientChange={setPatient}
      />
      <DiagnosisEditor
        key={`diagnosis-${formVersion}`}
        onDiagnosesChange={setDiagnoses}
      />
      <OrderDetailsPanel
        key={`order-${formVersion}`}
        patient={patient}
        diagnoses={diagnoses}
      />
    </div>
  );
}
