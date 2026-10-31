import { useState } from "react";
import AdminPage from "./features/admin/AdminPage";
import DailyOperationsPage from "./features/orders/DailyOperationsPage";
import homekureLogo from "./assets/homekure-logo.png";

type AppPage = "operations" | "admin";

export default function App() {
  const [page, setPage] = useState<AppPage>("operations");

  return (
    <main className="min-h-screen px-3 py-5 text-slate-900 sm:px-6 sm:py-8">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-5">
        <header className="rounded-xl border border-slate-200 bg-white px-5 py-5 text-center shadow-sm sm:px-8">
          <img
            className="mx-auto h-auto w-full max-w-[320px]"
            src={homekureLogo}
            alt="Homekure Doctors of America, Arlington, Texas"
          />
          <h1 className="mt-4 text-lg font-bold tracking-wide text-blue-950">
            PHYSICIAN’S ORDER
          </h1>
          <p className="mt-1 text-sm text-slate-600">
            Physician Order Automation
          </p>
        </header>
        <p className="text-center text-xs text-slate-500">
          Local workspace · patient records stay on this device
        </p>
        <nav
          className="flex flex-wrap gap-2 border-b border-slate-200"
          aria-label="Main navigation"
        >
          <button
            aria-current={page === "operations" ? "page" : undefined}
            className={`border-b-2 px-3 py-2 text-sm font-medium ${page === "operations" ? "border-blue-700 text-blue-800" : "border-transparent text-slate-600 hover:text-slate-900"}`}
            onClick={() => setPage("operations")}
            type="button"
          >
            Daily operations
          </button>
          <button
            aria-current={page === "admin" ? "page" : undefined}
            className={`border-b-2 px-3 py-2 text-sm font-medium ${page === "admin" ? "border-blue-700 text-blue-800" : "border-transparent text-slate-600 hover:text-slate-900"}`}
            onClick={() => setPage("admin")}
            type="button"
          >
            Administration
          </button>
        </nav>
        {page === "admin" ? <AdminPage /> : <DailyOperationsPage />}
      </div>
    </main>
  );
}
