import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { FormEvent } from "react";

type Physician = { id: string; name: string; npi: string; active: boolean; sortOrder: number };
type OrderType = { id: string; name: string; mdOrderBody: string; active: boolean; sortOrder: number };
type PhysicianDraft = Omit<Physician, "id"> & { id: string | null };
type OrderTypeDraft = Omit<OrderType, "id"> & { id: string | null };

const newPhysician = (): PhysicianDraft => ({ id: null, name: "", npi: "", active: true, sortOrder: 0 });
const newOrderType = (): OrderTypeDraft => ({ id: null, name: "", mdOrderBody: "", active: true, sortOrder: 0 });
const inputClass = "w-full rounded-md border border-slate-300 px-3 py-2 text-sm outline-none focus:border-blue-600 focus:ring-2 focus:ring-blue-100";
const labelClass = "flex flex-col gap-1 text-sm font-medium text-slate-700";

export default function ReferenceDataPanel() {
  const [physicians, setPhysicians] = useState<Physician[]>([]);
  const [orderTypes, setOrderTypes] = useState<OrderType[]>([]);
  const [physician, setPhysician] = useState<PhysicianDraft>(newPhysician());
  const [orderType, setOrderType] = useState<OrderTypeDraft>(newOrderType());
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    const [physicianRows, orderTypeRows] = await Promise.all([
      invoke<Physician[]>("list_physicians"),
      invoke<OrderType[]>("list_order_types"),
    ]);
    setPhysicians(physicianRows);
    setOrderTypes(orderTypeRows);
  }, []);

  useEffect(() => {
    refresh().catch((reason: unknown) => setMessage(String(reason)));
  }, [refresh]);

  async function savePhysician(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    try {
      const saved = await invoke<Physician>("save_physician", { physician });
      setMessage(`${saved.name} saved.`);
      setPhysician(newPhysician());
      await refresh();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function saveOrderType(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    try {
      const saved = await invoke<OrderType>("save_order_type", { orderType });
      setMessage(`${saved.name} saved.`);
      setOrderType(newOrderType());
      await refresh();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function deletePhysician(item: Physician) {
    if (!window.confirm(`Delete ${item.name}?`)) return;
    setBusy(true);
    setMessage("");
    try {
      await invoke("delete_physician", { physicianId: item.id });
      setMessage(`${item.name} deleted.`);
      await refresh();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function deleteOrderType(item: OrderType) {
    if (!window.confirm(`Delete the ${item.name} order type?`)) return;
    setBusy(true);
    setMessage("");
    try {
      await invoke("delete_order_type", { orderTypeId: item.id });
      setMessage(`${item.name} deleted.`);
      await refresh();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="rounded-lg border border-slate-200 bg-white p-6 shadow-sm">
      <div>
        <h2 className="text-lg font-medium">Practice reference data</h2>
        <p className="mt-1 text-sm text-slate-600">Manage the physicians and reusable order templates available to staff.</p>
      </div>
      {message && <p role="status" className="mt-4 rounded bg-slate-50 p-3 text-sm text-slate-700">{message}</p>}

      <section className="mt-6 border-t border-slate-200 pt-5">
        <h3 className="font-medium">Physicians</h3>
        <ul className="mt-3 divide-y divide-slate-200 rounded-md border border-slate-200">
          {physicians.map((item) => (
            <li className="flex items-center justify-between gap-3 p-3" key={item.id}>
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{item.name}</p>
                <p className="text-xs text-slate-600">NPI {item.npi} · {item.active ? "Active" : "Inactive"}</p>
              </div>
              <div className="flex shrink-0 gap-2">
                <button className="text-sm text-blue-700 hover:underline" type="button" onClick={() => setPhysician({ ...item })}>Edit</button>
                <button className="text-sm text-red-700 hover:underline disabled:opacity-50" disabled={busy} type="button" onClick={() => void deletePhysician(item)}>Delete</button>
              </div>
            </li>
          ))}
          {physicians.length === 0 && <li className="p-3 text-sm text-slate-500">No physicians added yet.</li>}
        </ul>
        <form className="mt-4 grid gap-3 sm:grid-cols-2" onSubmit={savePhysician}>
          <label className={labelClass}>Physician name
            <input className={inputClass} required maxLength={200} value={physician.name} onChange={(event) => setPhysician((current) => ({ ...current, name: event.target.value }))} />
          </label>
          <label className={labelClass}>NPI
            <input className={inputClass} required inputMode="numeric" pattern="[0-9]{10}" maxLength={10} value={physician.npi} onChange={(event) => setPhysician((current) => ({ ...current, npi: event.target.value }))} />
          </label>
          <label className={labelClass}>Display order
            <input className={inputClass} type="number" min={0} value={physician.sortOrder} onChange={(event) => setPhysician((current) => ({ ...current, sortOrder: Number(event.target.value) }))} />
          </label>
          <label className="flex items-center gap-2 text-sm text-slate-700">
            <input type="checkbox" checked={physician.active} onChange={(event) => setPhysician((current) => ({ ...current, active: event.target.checked }))} />
            Active
          </label>
          <div className="flex gap-2 sm:col-span-2">
            <button className="rounded-md bg-blue-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50" disabled={busy}>{physician.id ? "Update physician" : "Add physician"}</button>
            {physician.id && <button className="rounded-md border border-slate-300 px-4 py-2 text-sm" type="button" onClick={() => setPhysician(newPhysician())}>Cancel edit</button>}
          </div>
        </form>
      </section>

      <section className="mt-7 border-t border-slate-200 pt-5">
        <h3 className="font-medium">Order types and templates</h3>
        <ul className="mt-3 divide-y divide-slate-200 rounded-md border border-slate-200">
          {orderTypes.map((item) => (
            <li className="flex items-center justify-between gap-3 p-3" key={item.id}>
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{item.name}</p>
                <p className="text-xs text-slate-600">{item.active ? "Active" : "Inactive"} · {item.mdOrderBody ? "Template set" : "No template text"}</p>
              </div>
              <div className="flex shrink-0 gap-2">
                <button className="text-sm text-blue-700 hover:underline" type="button" onClick={() => setOrderType({ ...item })}>Edit</button>
                <button className="text-sm text-red-700 hover:underline disabled:opacity-50" disabled={busy} type="button" onClick={() => void deleteOrderType(item)}>Delete</button>
              </div>
            </li>
          ))}
          {orderTypes.length === 0 && <li className="p-3 text-sm text-slate-500">No order types added yet.</li>}
        </ul>
        <form className="mt-4 flex flex-col gap-3" onSubmit={saveOrderType}>
          <label className={labelClass}>Order type name
            <input className={inputClass} required maxLength={120} value={orderType.name} onChange={(event) => setOrderType((current) => ({ ...current, name: event.target.value }))} />
          </label>
          <label className={labelClass}>Reusable MD order template
            <textarea className={`${inputClass} min-h-28`} maxLength={20000} value={orderType.mdOrderBody} onChange={(event) => setOrderType((current) => ({ ...current, mdOrderBody: event.target.value }))} />
          </label>
          <label className={labelClass}>Display order
            <input className={inputClass} type="number" min={0} value={orderType.sortOrder} onChange={(event) => setOrderType((current) => ({ ...current, sortOrder: Number(event.target.value) }))} />
          </label>
          <label className="flex items-center gap-2 text-sm text-slate-700">
            <input type="checkbox" checked={orderType.active} onChange={(event) => setOrderType((current) => ({ ...current, active: event.target.checked }))} />
            Active
          </label>
          <div className="flex gap-2">
            <button className="rounded-md bg-blue-700 px-4 py-2 text-sm font-medium text-white disabled:opacity-50" disabled={busy}>{orderType.id ? "Update order type" : "Add order type"}</button>
            {orderType.id && <button className="rounded-md border border-slate-300 px-4 py-2 text-sm" type="button" onClick={() => setOrderType(newOrderType())}>Cancel edit</button>}
          </div>
        </form>
      </section>
    </section>
  );
}
