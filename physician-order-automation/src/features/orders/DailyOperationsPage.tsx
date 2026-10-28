import DiagnosisEditor from "./DiagnosisEditor";
import PatientRecordsPanel from "../patients/PatientRecordsPanel";

export default function DailyOperationsPage({ isAdmin }: { isAdmin: boolean }) {
  return (
    <div className="flex flex-col gap-6">
      <header>
        <h2 className="text-xl font-semibold">Daily operations</h2>
        <p className="mt-1 text-sm text-slate-600">Work with patient demographics and prepare order details.</p>
      </header>
      <PatientRecordsPanel isAdmin={isAdmin} />
      <DiagnosisEditor />
    </div>
  );
}
