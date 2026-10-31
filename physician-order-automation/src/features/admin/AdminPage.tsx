import ReferenceDataPanel from "../reference-data/ReferenceDataPanel";

export default function AdminPage() {
  return (
    <div className="flex flex-col gap-6">
      <header className="rounded-xl border border-blue-200 bg-blue-950 p-5 text-white">
        <h2 className="text-xl font-semibold">Administration</h2>
        <p className="mt-1 text-sm text-white">
          Manage practice reference data used by daily operations.
        </p>
      </header>
      <ReferenceDataPanel />
    </div>
  );
}
