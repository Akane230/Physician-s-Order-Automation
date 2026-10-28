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

export default function DailyOperationsPage({ isAdmin }: { isAdmin: boolean }) {
  const [patient, setPatient] = useState<PatientSelection>(emptyPatient);
  const [diagnoses, setDiagnoses] = useState<Diagnosis[]>([]);

  return (
    <div className="flex flex-col gap-6">
      <header>
        <h2 className="text-xl font-semibold">Daily operations</h2>
        <p className="mt-1 text-sm text-slate-600">Work with patient demographics and prepare order details.</p>
      </header>
      <PatientRecordsPanel isAdmin={isAdmin} onPatientChange={setPatient} />
      <DiagnosisEditor onDiagnosesChange={setDiagnoses} />
      <OrderDetailsPanel patient={patient} diagnoses={diagnoses} />
    </div>
  );
}
