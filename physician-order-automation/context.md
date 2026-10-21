# Project Context: Physician's Order Automation (Desktop App)

> Single source of truth for the project's goals, constraints, decisions, and status. Keep it updated as decisions change. Items marked **[PENDING]** still need client confirmation or action.
>
> Last updated: 2026-10-21

---

## 1. Overview

The client is a home-care physician practice (Homekure Doctors of America, Arlington, TX). Staff currently fill out a **Physician's Order** PDF by hand. This app automates that process.

**Workflow**
1. Staff enter or paste patient demographics, diagnoses, choose the ordering physician and order type, and add optional comments.
2. Clicking **Generate order** saves the demographics to the local database and produces the Physician's Order PDF.
3. Staff can also save demographics on their own with the **Save this patient to records** button.
4. The PDF is exported (downloaded) to a location the user chooses.

**Deliverable:** an installable desktop application (Windows and macOS), not a web app.

---

## 2. Hard Requirements

| Requirement | Detail |
|---|---|
| Fully local | No data leaves the machine. Database is local. No telemetry, CDN assets, remote fonts, or auto-updater. |
| Desktop app | Installer the client can run on their computers. |
| Platforms | **Windows and macOS** |
| Users | **Single workstation for now.** May scale to a secured LAN later, so the architecture must allow it. |
| Lightweight | Staff keep many tabs/apps open, so the app must be low on RAM/CPU and idle quietly. |
| Security | US patient data (PHI), so HIPAA-oriented design. Client should confirm compliance requirements with their own HIPAA officer/advisor. |
| Admin side | Admin can freely CRUD physicians, order types and their MD order templates, users, etc. |
| Order content not stored | Only **demographics** are persisted. Diagnoses, comments, and MD order text are never saved to the DB, logs, or temp files. |
| PDF export | Generated PDF must be downloadable. The user chooses the save location (native Save As dialog every time). |
| Signature | Digital signature is handled by a **separate system** the client already uses. The "Physician Signature" box stays **blank** on our PDF. |
| Deployment | The developer handles installation and packaging; no client IT involvement. |

---

## 3. Form Design (from UI screenshots)

**1. Patient Demographics**
- "Load a saved patient" dropdown, **Save this patient to records**, **Delete saved patient**
- Paste box: *"Paste patient demographics from your EHR / intake sheet"* with **Auto-fill fields below** and **Clear** (accepts labeled or unformatted text)
- Fields: Patient name, Gender (select), Address, DOB (MM/DD/YYYY), Phone no., Medicare, Other insurance, Order date (date picker, defaults to today)
- Diagnoses: paste in any format (one per line or a full block), e.g. `ICD-10-CM Condition: G81.94 Hemiplegia, unspecified affecting left non-dominant side`
- Buttons: **Clean up list**, **Suggest top 5 relevant to this order**

**2. Physician**: "Ordering physician" dropdown (admin-managed)

**3. Order Type**: "Select the type of order" dropdown (admin-managed; each type has its own MD order text)

**4. Comments**: free text

**Actions:** **Generate order**, **Start new order** (clears all per-order state)

Note: the screenshot shows the message *"Patient records need sign-in, which isn't available in this view."* Local sign-in is part of the plan.

---

## 4. PDF Template

Provided as `PO_Format.pdf` (a single page, **not** a fillable form).

**Header:** Homekure Doctors of America logo and address/phone/fax, "PHYSICIAN'S ORDER" title.

**Fields, top to bottom:** Patient Name, Address, DOB, Phone No., Medicare, Other Ins, Order Date, Physician, Diagnoses, **MD ORDER** (large body area), Comments, Physician Signature (with printed name and NPI), footer `www.homekure.com`.

**Template issues to handle**
- The **order date** (`10/13/2026`) and **physician** (`Dr. Holly Canaday NPI: 1255879581`) are baked into the template as static text. Both are dynamic in the app, so strip them from the base template (ideally get a clean source file from the client).
- The printed name/NPI under "Physician Signature" must come from the selected physician. The signature area itself stays blank.
- Diagnoses and the MD ORDER body vary in length, so **support overflow to a second page**, repeating the header and patient name on continuation pages.

**Approach:** use a cleaned template as the base layer and overlay dynamic text with custom wrapping logic using font metrics. Fallback: rebuild the template as HTML/CSS. Prove this early; it's the riskiest piece.

**Export filename:** `PhysicianOrder_LastName_FirstInitial_YYYY-MM-DD.pdf`. Do not put the full DOB in filenames.

---

## 5. Tech Stack

| Layer | Choice |
|---|---|
| Shell | **Tauri 2** (OS webview, small footprint; Electron was rejected as heavier) |
| Frontend | React + TypeScript + Vite |
| UI/state | Tailwind, TanStack Query, react-hook-form + zod (no heavy component library) |
| Backend | Rust (DB, PDF, parsing, crypto, audit all live here; the webview only does UI) |
| Database | SQLite + **SQLCipher** (`rusqlite`, feature `bundled-sqlcipher-vendored-openssl`), WAL mode |
| Passwords | Argon2id |
| Key storage | OS keychain via `keyring` (Windows Credential Manager, macOS Keychain) |
| PDF | `lopdf` or `pdf-writer` + `ttf-parser` (final choice after testing against the template) |
| Parsing | Rust `regex` + heuristics (deterministic and auditable) |
| ICD-10-CM | Bundled in SQLite with FTS5 search (~70k codes) |
| OCR (later) | Tesseract or `ocrs` as an on-demand sidecar |

**Fallback if Rust or Tauri becomes a blocker:** Electron + `better-sqlite3-multiple-ciphers` with the same overall design.

### Workspace layout

```
physician-order-automation/        (codebase root: contains Cargo.toml, package.json)
├─ Cargo.toml                      (workspace: members = ["src-tauri", "crates/core"])
├─ package.json
├─ src/                            (React)
├─ src-tauri/                      (thin Tauri shell: commands only)
│  └─ resources/                   (clean PDF template, fonts)
└─ crates/
   └─ core/                        (crate name: orders_core; all real logic)
      └─ src/ { db, auth, patients, parsing, icd, pdf, audit, backup }
```

The repo's git root is one level up (`Physician-s-Order-Automation/`, which also holds `.git` and `.vs/`). Because this is a Cargo workspace, build output goes to the **workspace-root** `target/`, not `src-tauri/target/`.

The core crate is named `orders_core` (not `core`) to avoid colliding with Rust's built-in `core`. Business logic sits behind repository traits so a future LAN `server` crate (axum, mTLS) can reuse `orders_core` and the desktop app can switch from a local repo to a remote repo.

---

## 6. Data Model (SQLCipher)

```sql
users(id, username UNIQUE, display_name, password_hash /*Argon2id*/,
      role CHECK IN ('admin','staff'), active, failed_attempts, locked_until,
      must_change_password, created_at, updated_at)

patients(id, last_name, first_name, full_name NOT NULL, gender, address,
         dob /*ISO YYYY-MM-DD; display MM/DD/YYYY*/, phone, medicare, other_insurance,
         created_at, updated_at, created_by, updated_by)
-- indexes: UNIQUE(medicare) WHERE medicare IS NOT NULL; (last_name, dob)

physicians(id, name, npi, active, sort_order)
order_types(id, name, md_order_body, active, sort_order)
order_type_icd(order_type_id, icd_code, rank)      -- powers "Suggest top 5"

icd_codes(code PK, description, fiscal_year)
icd_fts                                             -- FTS5 over code + description

settings(key PK, value)

audit_log(id, ts, user_id, action, entity_type, entity_id, detail, prev_hash, hash)
```

**Notes**
- **No orders table and no diagnoses table.** The exported PDF is the only record of an order.
- `md_order_body` supports simple placeholders (e.g. `{{patient_name}}`, `{{order_date}}`) so admins can edit templates without code changes.
- **ICD-10-CM updates each October 1.** Ship the current fiscal-year set and provide an admin offline "import code update" function.
- **Patient matching:** Medicare ID first, then last name + first name + DOB. Normalize before comparing (trim, case-fold, strip punctuation from phone/ID).
- Migrations are versioned from day one (a `schema_version` table plus ordered SQL strings).
- Audit `detail` holds metadata only. Never log diagnoses, comments, or PHI beyond the patient ID.

---

## 7. Tauri Command Surface

Every command checks session and role in Rust. The frontend never decides permissions.

| Area | Commands | Role |
|---|---|---|
| Auth | `login`, `logout`, `lock`, `unlock`, `change_password`, `session_status` | all |
| Patients | `search_patients`, `get_patient`, `save_patient` (returns `created`/`updated`/`conflict`), `delete_patient` | staff; delete = admin |
| Parsing | `parse_demographics(text)`, `parse_diagnoses(text)` | staff |
| ICD | `search_icd(query)`, `normalize_diagnoses(list)`, `suggest_icd(order_type_id)` | staff |
| Reference data | `list_physicians`, `list_order_types` | staff |
| Admin CRUD | upsert/delete for physicians, order types, order-ICD mappings, users | admin |
| PDF | `render_order(payload)` returns bytes for preview; `export_order(payload, path)` | staff |
| Backup | `export_backup(path, passphrase)`, `import_backup(path, passphrase)` | admin |
| Audit | `list_audit(filters)`, `verify_audit_chain` | admin |
| Settings | `get_settings`, `update_settings` | admin |

**Generate order flow:** `save_patient` then `render_order`/`export_order`. On `conflict`, pause and ask the user before generating. Only `save_patient` writes patient data.

The Save As dialog is invoked from inside the Rust export command, and Rust writes only to the returned path, so the frontend never gets arbitrary file-write access.

---

## 8. Security and Privacy Rules

- Encrypted DB at rest (SQLCipher). The key lives in the OS keychain, never beside the DB or hardcoded.
- **Backup needs its own passphrase-derived key**, since a keychain key is tied to the OS user account and can't restore on another machine.
- Local sign-in with roles (admin/staff), lockout on repeated failures, and inactivity auto-lock.
- First-run setup screen creates the initial admin. **No default password is shipped.**
- Strict CSP; no remote anything. Disable DevTools in release builds.
- Tauri capabilities on a minimal allowlist. **Do not enable** shell, http, fs, or updater plugins.
- Zeroize secrets in memory (`zeroize`).
- Hash-chained audit log for tamper evidence.
- "Start new order" must clear diagnoses and comments from memory, not just the UI. No localStorage, query-cache persistence, crash-log capture, or temp files for order content. Previews stay in memory.
- Exported PDFs are unencrypted files containing PHI; once exported they're outside the app's control. The client should decide on handling (designated folder, print only, etc.).
- Keep real patient data out of the repo and out of test fixtures (use fake names). Keep the DB file, exported PDFs, and `.vs/` out of git.

---

## 9. Behavior Decisions

- **Auto-save on Generate:** upsert by match key; tell the user "saved new patient" vs. "updated existing patient."
- **Edited demographics on Generate:** if the form differs from the saved record, prompt: *Update saved record* or *Use for this order only*.
- **Diagnosis suggestions:** curated order-type-to-ICD mapping only (offline, predictable, auditable). No ML model in v1.
- **"Clean up list":** validate and normalize codes against the bundled ICD table.
- **Demographics parsing:** rule-based; always show the result for human review before saving.
- **Audit entry per generated order** (user, patient ID, order type, timestamp, no clinical content): **[PENDING client confirmation]**

---

## 10. Decisions Log

| # | Decision | Status |
|---|---|---|
| 1 | Tauri 2 + React/TS + Rust instead of Electron | Decided |
| 2 | SQLite + SQLCipher, local only | Decided |
| 3 | Windows and macOS targets | Confirmed by client |
| 4 | Single workstation now; LAN sharing possible later | Confirmed |
| 5 | Orders are not stored in DB; only demographics | Confirmed |
| 6 | PDF is downloadable; user picks the location each time | Confirmed |
| 7 | Digital signature handled elsewhere; leave blank | Confirmed |
| 8 | Admin can CRUD physicians, order types, users | Confirmed |
| 9 | Developer handles installs (no client IT) | Confirmed |
| 10 | No order content retained anywhere in the app | Confirmed |
| 11 | Audit log entry per generated order | **PENDING** |
| 12 | Clean template source file (without baked-in date/physician) | **PENDING** (ask client) |
| 13 | Order-type list and MD order text for each type | **PENDING** (ask client) |
| 14 | Starter physician list with NPIs | **PENDING** (ask client) |

---

## 11. Future Work: Import / OCR

The client's work platform can export patient data. **Waiting on a sample export file.**

- If the export is CSV/XLSX/text-based PDF: parse directly, no OCR needed (more reliable).
- If it's scanned images or image-only PDFs: bundle Tesseract (or `ocrs`) locally, and require a review screen before saving because OCR errors in patient data are a real risk.

---

## 12. Packaging

- **Windows:** signed MSI/NSIS installer (a code-signing certificate is a budget item; unsigned triggers SmartScreen). Bundle the WebView2 offline installer for locked-down machines.
- **macOS:** universal binary (Apple Silicon + Intel), signed and notarized (Apple Developer account, ~$99/year). Build on a Mac or CI runner. `rustup target add aarch64-apple-darwin x86_64-apple-darwin`, then `npm run tauri build -- --target universal-apple-darwin`.
- No internet means updates are manual installer files, so DB migrations must be versioned and safe.
- Test the UI on both platforms early (WebView2/Chromium vs. WebKit rendering differences).
- Release installers land in `target/release/bundle/` at the **workspace root** (`msi/` and `nsis/`).

---

## 13. Build Order

1. Cargo workspace, Tauri shell, encrypted DB with migrations, local auth
2. Patients and the demographics form with paste-and-parse
3. **PDF engine on the clean template**, including overflow to page 2 (riskiest, so do it early)
4. Physicians, order types, and templates admin
5. ICD search, cleanup, and suggestions
6. Audit log, backup/restore, auto-lock
7. Packaging, signing, Windows/Mac testing
8. OCR/import, once the sample file arrives

---

## 14. Dev Environment

**Machine:** Windows dev machine. Editor: **VS Code** (preferred over the Visual Studio IDE; Visual Studio is installed only to provide the MSVC compiler and Windows SDK).

**Toolchain installed**
- Rust (MSVC toolchain), Node.js, Tauri
- Visual Studio Community with the "Desktop development with C++" workload (MSVC 14.51)
- **Strawberry Perl** (`C:\Strawberry\perl\bin`), required to build vendored OpenSSL for SQLCipher
- NASM is not needed (OpenSSL builds fine without it)

**VS Code extensions:** rust-analyzer, Tauri, CodeLLDB (or C/C++), Even Better TOML, Tailwind CSS IntelliSense, ESLint, Prettier. Open the **codebase root** (`physician-order-automation/`) as the folder, not `src-tauri`. After installing anything that changes PATH, fully restart VS Code.

**Paths**
- Git root: `C:\Users\Admin\Documents\Physician-s-Order-Automation\` (contains `.git` and `.vs/`)
- Codebase root: `...\Physician-s-Order-Automation\physician-order-automation\` (contains `Cargo.toml`, `package.json`, `crates/`, `src-tauri/`)

### Build workaround (important)

Building OpenSSL inside `Documents` failed twice: first with `fatal error C1090: PDB API call failed`, then with `os error 32: The process cannot access the file because it is being used by another process`. Both are file-lock problems (likely OneDrive/Defender scanning, or rust-analyzer building concurrently) made worse by long paths.

**What worked:** build into a short, non-synced folder.

```powershell
mkdir C:\t -ErrorAction SilentlyContinue
$env:CARGO_TARGET_DIR = "C:\t"
cargo test -p orders_core -- --nocapture
```

`$env:CARGO_TARGET_DIR` lasts only for the current terminal session. The first OpenSSL build took about 9 minutes; later builds reuse the cache.

Also applied: Windows Defender exclusions for the codebase folder and `C:\t`:

```powershell
Add-MpPreference -ExclusionPath "C:\Users\Admin\Documents\Physician-s-Order-Automation\physician-order-automation"
Add-MpPreference -ExclusionPath "C:\t"
```

**Permanent fix [RECOMMENDED, PENDING]:** move the repo out of `Documents` to a short non-synced path. Best done now, before much code exists. Reasons: `$env:CARGO_TARGET_DIR` resets with each new terminal and rust-analyzer ignores it unless configured, so it may silently rebuild OpenSSL into `Documents\...\target` and hit the lock errors again; OneDrive may sync thousands of build files (check with `echo $env:OneDrive`) and could also sync a project that will later hold a database and test patient data; and the path is long, which the C toolchain handles poorly.

Steps (close VS Code and any `cargo`/`tauri dev` terminals first):

```powershell
mkdir C:\dev -ErrorAction SilentlyContinue
Move-Item "C:\Users\Admin\Documents\Physician-s-Order-Automation" "C:\dev\Physician-s-Order-Automation"
Remove-Item -Recurse -Force "C:\dev\Physician-s-Order-Automation\physician-order-automation\target" -ErrorAction SilentlyContinue
Add-MpPreference -ExclusionPath "C:\dev"
```

Then reopen VS Code from `C:\dev\Physician-s-Order-Automation\physician-order-automation` and build without `CARGO_TARGET_DIR` (first build recompiles OpenSSL, about 9 minutes, then cached). If the same file-lock errors return, keep using `C:\t` and investigate other causes. Update the paths in this file after moving.

**If not moving yet:** set the target dir permanently for the user so every terminal and VS Code uses it, then restart VS Code:

```powershell
setx CARGO_TARGET_DIR "C:\t"
```

**Optional safety net:** if OpenSSL builds become a recurring problem, make `rusqlite` an optional `db` feature in `orders_core` (`default = []`, `db = ["dep:rusqlite"]`, gate the db module and test with `#[cfg(feature = "db")]`) so non-database work builds without it. Not needed while the current build works.

### Cargo notes
- Warning seen on every build: *"profiles for the non root package will be ignored"*. The Tauri template put `[profile.*]` in `src-tauri/Cargo.toml`. **[PENDING]** Move any profile blocks to the workspace-root `Cargo.toml`. Suggested release profile: `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.
- Core crate dependencies added: `rusqlite` (bundled-sqlcipher-vendored-openssl), `argon2`, `zeroize`, `serde` (derive), `serde_json`, `thiserror`, `regex`, `chrono`, `sha2`, `keyring`, `lopdf`; dev-dependency `tempfile`.
- **Dialog plugin (Save As):** `src-tauri/src/lib.rs` registers `.plugin(tauri_plugin_dialog::init())`, which requires the crate in `src-tauri/Cargo.toml`. A build error `cannot find module or crate tauri_plugin_dialog` means it was never added. Fix (run from the codebase root): `cargo add tauri-plugin-dialog -p physician-order-automation`. The Cargo package name of the Tauri shell is `physician-order-automation`.
- **Do not** `npm install @tauri-apps/plugin-dialog` and do not grant the frontend a dialog permission in `src-tauri/capabilities/default.json`. The Save As dialog is opened from inside the Rust export command so the frontend never gets file-write access. Planned usage: `use tauri_plugin_dialog::DialogExt;` then `app.dialog().file().add_filter("PDF", &["pdf"]).blocking_save_file()` inside a command that receives `app: tauri::AppHandle`.
- `crates/core/src/lib.rs` currently contains a `ping()` function and the SQLCipher test described below (the default `cargo new` `add` function was removed).

---

## 15. Current Status

### Smoke tests

| # | Test | Status |
|---|---|---|
| 1 | **SQLCipher encryption** (`cargo test -p orders_core -- --nocapture`) | **PASSED** (2026-10-21) |
| 2 | Rust-to-React command bridge (`ping` via `invoke`, validates CSP) | Not run yet |
| 3 | Release build (`npm run tauri build`), then install and check size/RAM | Not run yet |

**Test 1 result:** `cipher_version = 4.14.0 community`, `test result: ok. 1 passed`. The test confirms SQLCipher is genuinely linked, the DB file does not start with the plain `SQLite format 3` header, a wrong key cannot read the data, and the correct key can. The three `ERROR CORE sqlcipher_page_cipher: hmac check failed` log lines are **expected**: they're SQLCipher rejecting the deliberately wrong key. One harmless `linker_messages` warning also appears.

### Handy commands
- Fast compile check of the Tauri shell without producing a binary: `cargo check -p physician-order-automation`
- Run the app in dev mode (compiles the Rust side itself, so a separate `cargo build` is not needed): `npm run tauri dev`
- Core tests: `cargo test -p orders_core -- --nocapture`
- Use the same terminal where `CARGO_TARGET_DIR` is set (or set it permanently with `setx`), otherwise Cargo builds into the project's own `target/` and recompiles OpenSSL.

### Next steps
1. Run smoke tests 2 and 3 (bridge and a release build; note installer size and RAM baseline).
2. Re-run `npm run tauri dev` after adding `tauri-plugin-dialog` (fixes the `E0433` error) and confirm the starter window opens.
3. Resolve the pending housekeeping: move repo to a short path, move `[profile]` blocks to the root `Cargo.toml`, add `.vs/` and `target/` to `.gitignore`.
4. Start the PDF engine on the cleaned template (can proceed in parallel; it doesn't depend on SQLCipher) and/or the database layer: versioned migrations, the `db` module opening SQLCipher with a keychain-stored key, and first-run admin setup.

---

## 16. Open Questions for the Client

1. Confirm the audit entry per generated order (metadata only).
2. Provide the clean template source (no baked-in date/physician).
3. Provide the order types and their MD order text.
4. Provide the physician list with NPIs.
5. Provide a sample export file from their work platform (for the import/OCR feature).
6. How will exported PDFs be handled after export (folder, print, upload elsewhere)?
7. Does a HIPAA officer or compliance advisor want input on encryption and access controls?