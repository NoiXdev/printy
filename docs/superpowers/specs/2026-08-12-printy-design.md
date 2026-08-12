# Printy — Design Spec

Date: 2026-08-12
Status: Approved (brainstorming complete, ready for implementation planning)

## 1. Purpose

Printy watches a set of local folders and automatically prints every new file
that appears in them. It targets office workstations where a scanner, an ERP
export or a label generator drops files into a folder and someone currently has
to open and print each one by hand.

Primary platform is Windows. macOS is supported as a development platform so the
application can be built and operated on the author's machine.

## 2. Scope

### In scope

- Watch N local folders, each with its own poll interval and print configuration
- Print PDF, JPG, PNG and TIFF
- Per-folder print settings: target printer, number of copies, duplex, colour mode
- Per-folder post-print rule: move to subfolder / keep and remember / delete
- Enumerate system printers and their capabilities, offer them in the UI
- Tray icon with state indication, background operation, autostart on login
- Desktop notifications (all events / errors only / off)
- Global pause switch
- Job history with retry

### Out of scope (deliberate)

- Network shares, UNC paths and cloud-sync folders. Only local paths are
  supported. Event-based watching is not implemented; polling is the sole
  mechanism.
- Recursive watching. A folder's subdirectories are not scanned. This keeps the
  `printed/` and `failed/` exclusion trivial and avoids surprise print storms.
- Office formats (DOCX, XLSX) and raw/ZPL spooling.
- Paper tray selection and paper size controls.

### Amended after review (2026-08-12): per-folder fit mode

Scaling is **not** out of scope. Each folder carries a `fit_to_page` flag:

- **On (default):** content is scaled to fill the printable area, preserving
  aspect ratio and centred. This is what people expect from "print this".
- **Off:** content is printed at its natural size, centred. Labels, receipts and
  small scans stay crisp instead of being blown up into a soft full-page image.

**Content larger than the printable area is shrunk to fit in both modes.** "Off"
means *never enlarge*, not *never scale* — printing an oversized page at natural
size would push content off the paper, which is data loss, not fidelity.

The flag is snapshotted onto the job at enqueue time, like the printer and copy
settings, so changing a folder's configuration never retroactively alters jobs
already waiting in the queue.
- Multi-user or server operation. Printy is a single-user desktop application.

## 3. Stack

Mirrors the existing `tabs-manager` (tabsy) project so both codebases stay
familiar:

- Tauri 2, Rust 2021
- React 19 + TypeScript + Vite, React Router, TanStack Query
- SQLite via `sqlx` with migrations
- `pdfium-render` for PDF rasterisation, `image` for bitmap decoding
- `windows` crate for the Win32 printing path
- Tauri plugins: `notification`, `autostart`, `dialog`, `opener`
- Tests: Rust unit/integration tests, Vitest + Testing Library for the frontend

## 4. Branding

Name: **Printy**. Sibling to tabsy, same design language, own accent.

| Token | Value | Role |
| --- | --- | --- |
| `--coral` | `#ff7a59` | Lead accent: primary buttons, active nav, brand |
| `--mint` | `#2fe6b7` | Success state only (running, printed) |
| `--teal` | `#0a2826` | Sidebar background, ink |
| `--cream` | `#f4f1ea` | Page background |
| `--surface` | `#fffdf8` | Cards |

Typography, radii (16px / 12px), shadows, light+dark theming and the sidebar
shell are taken over from tabsy's `theme.css` unchanged. Display font is
Fraunces Variable, self-hosted via `@fontsource-variable/fraunces`.

Logo: a teal folder with a coral sheet emerging from it and a mint confirmation
badge. Delivered as a React `Logo.tsx` component (mirroring tabsy's) plus the
full Windows icon set (`.ico`, all sizes) generated through the Tauri icon
pipeline.

UI strings are German. Code, comments and internal documentation are English.

## 5. Architecture

### 5.1 Rust modules (`src-tauri/src`)

| Module | Responsibility |
| --- | --- |
| `watcher` | One Tokio task per enabled folder, scanning on its own interval |
| `intake` | Decides whether a discovered file is ready to print (extension, stability, deduplication) |
| `queue` | Serial job queue, state machine, retry and backoff |
| `render` | PDF and image → RGB bitmaps at printer DPI (Windows path only) |
| `print` | `PrintBackend` trait plus `windows_gdi`, `sumatra`, `macos_cups`, `fake` |
| `printers` | Printer enumeration and capability probing |
| `db` | Schema, migrations, queries |
| `commands` | Tauri command surface for the frontend |
| `shell` | Tray icon, autostart, notifications |

### 5.2 Data flow

1. The watcher tick lists the folder (non-recursive), skipping the `printed/`
   and `failed/` subdirectories.
2. Entries whose extension is not in the folder's configured type list are
   discarded.
3. **Stability check.** A file is only considered complete when its size *and*
   mtime are unchanged across two consecutive ticks **and** it can be opened
   exclusively. This is what prevents printing a PDF a scanner is still writing.
4. **Deduplication** against the job ledger.
5. The file is enqueued as a job.
6. The queue worker prints jobs **strictly serially**. Parallel printing would
   interleave pages of different documents on the same printer.
7. On success the folder's post-print rule is applied.

### 5.3 Concurrency model

- One Tokio task per folder for scanning; scanning is I/O-light and independent.
- Exactly **one** queue worker for the whole application. Job ordering is FIFO by
  enqueue time.
- The database is the single source of truth. All state transitions are persisted
  before the next step begins.

## 6. Data model

Three tables. SQL is illustrative; exact DDL lives in the migrations.

### `watch_folders`

| Column | Type | Notes |
| --- | --- | --- |
| `id` | INTEGER PK | |
| `name` | TEXT | User-facing label |
| `path` | TEXT | Absolute local path, unique |
| `enabled` | BOOLEAN | Per-folder pause |
| `poll_interval_secs` | INTEGER | Default 5, floor 1 |
| `file_types` | TEXT | JSON array, e.g. `["pdf","png"]` |
| `printer_name` | TEXT | System printer name |
| `copies` | INTEGER | Default 1 |
| `duplex` | TEXT | `simplex` \| `long_edge` \| `short_edge` |
| `color_mode` | TEXT | `color` \| `mono` |
| `post_action` | TEXT | `move` \| `keep` \| `delete` |
| `status` | TEXT | `ok` \| `path_missing` \| `printer_missing` |
| `created_at`, `updated_at` | TEXT | ISO 8601 |

### `print_jobs`

Serves as queue, history and deduplication ledger in one.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | INTEGER PK | |
| `folder_id` | INTEGER FK | |
| `file_path`, `file_name` | TEXT | Path at time of discovery |
| `size_bytes` | INTEGER | |
| `mtime` | TEXT | ISO 8601 |
| `sha256` | TEXT | Content hash, computed at enqueue |
| `state` | TEXT | See state machine |
| `attempts` | INTEGER | |
| `printer_name` | TEXT | Snapshot of config at enqueue |
| `error_kind` | TEXT | Nullable: `file` \| `printer` \| `config` \| `interrupted` |
| `error_message` | TEXT | Nullable |
| `enqueued_at`, `started_at`, `finished_at` | TEXT | Nullable |

Indexes on `(state)`, `(folder_id, sha256)` and `(folder_id, file_name, size_bytes, mtime)`.

### `settings`

Key-value table `app_setting (key TEXT PRIMARY KEY, value TEXT)`, matching tabsy's
existing settings pattern, with typed accessors that fall back to a default when
the key is absent or unparseable. Keys: `notification_mode` (`all` \| `errors` \|
`off`), `autostart`, `start_minimized`, `theme`, `sumatra_path`, `user_paused`.

There are **two distinct kinds of pause** and they must not be conflated:
`user_paused` is the persisted global switch the user toggles and it survives a
restart; the *printer hold* triggered by a printer-level error (section 8) is
runtime-only state that is never persisted and clears by itself when the printer
returns. The UI shows them differently: "Pausiert" versus "Wartet auf Drucker".

### Deduplication rule

- `post_action = move` or `delete`: the file leaves the folder, so re-discovery
  cannot occur. The hash is still recorded for the history.
- `post_action = keep`: the file stays. A candidate is skipped when a completed
  job exists for the same `(folder_id, sha256)`. The cheap
  `(folder_id, file_name, size_bytes, mtime)` index is consulted first to avoid
  hashing every file on every tick.

## 7. Job state machine

```
Discovered ──> Stabilizing ──> Queued ──> Printing ──> Done
                                  ^           │
                                  │           v
                                  └──── Retrying ────> Failed
```

- `Discovered` → `Stabilizing`: file matched the type filter.
- `Stabilizing` → `Queued`: size and mtime unchanged over two ticks and the file
  opens exclusively.
- `Queued` → `Printing`: the worker picked the job up.
- `Printing` → `Done`: backend reported the job was accepted by the spooler and
  the post-print rule was applied successfully.
- `Printing` → `Retrying`: retryable **file-level** error. Attempts are capped at
  3, with backoff 5 s, 30 s, 120 s.
- `Retrying` → `Failed`: attempts exhausted.

**Printer-level errors do not consume an attempt.** The job returns to `Queued`
unchanged and the queue enters the printer hold described in section 8. A printer
that is switched off must not burn through a job's three retries; when it comes
back, the job still has all of them.

**Crash recovery.** On startup any job still in `Printing` is moved to `Failed`
with `error_kind = interrupted`, never silently retried. A 40-page document must
not come out twice because the machine lost power mid-job. The user can reprint
it explicitly from the history.

## 8. Error taxonomy

The distinction matters more than it looks — it decides whether one job or the
whole queue is affected.

**File-level** (corrupt PDF, locked, unreadable, unsupported): affects only this
job. After three attempts the job goes to `Failed`, the post-print failure rule
applies (move to `failed/` when `post_action = move`), a notification is emitted,
and the queue continues.

**Printer-level** (offline, driver missing, out of paper, spooler error): affects
every job. The queue **pauses globally** and probes the printer once per minute,
resuming automatically when it returns. Without this, a switched-off printer would
push every file through three failed attempts within seconds and fill `failed/`
with dozens of files over a pulled plug.

**Config-level** (folder deleted, printer no longer installed): sets the affected
folder's `status` to a visible error state in the UI. One notification, then
silence — no repeat alerts on every tick.

## 9. Print backends

```rust
trait PrintBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>>;
    fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities>;
    fn print(&self, job: &PrintRequest) -> Result<()>;
}
```

`PrintRequest` carries the file path, printer name, copies, duplex mode and
colour mode. `PrinterCapabilities` reports whether the driver supports duplex,
colour and multiple copies, so the UI can disable controls the printer cannot
honour rather than silently ignoring them.

### 9.1 `windows_gdi` (primary, Windows)

- Enumerate printers with `EnumPrinters` (`PRINTER_ENUM_LOCAL |
  PRINTER_ENUM_CONNECTIONS`), default via `GetDefaultPrinter`.
- Capabilities via `DeviceCapabilities` (`DC_DUPLEX`, `DC_COLORDEVICE`,
  `DC_COPIES`).
- Obtain the `DEVMODE` through `OpenPrinter` + `DocumentProperties`, set
  `dmCopies`, `dmDuplex` and `dmColor` with the matching `dmFields` flags.
- `CreateDC` with the modified `DEVMODE`.
- Query `PHYSICALWIDTH`/`PHYSICALHEIGHT`, `PHYSICALOFFSETX`/`Y` and
  `LOGPIXELSX`/`Y` via `GetDeviceCaps` to compute the printable area.
- For each page: `StartPage`, rasterise via pdfium at the target DPI (capped at
  300 to bound memory — roughly 26 MB per A4 page), `StretchDIBits` into the
  printable area preserving aspect ratio, `EndPage`.
- Wrapped in `StartDoc` / `EndDoc`; `DeleteDC` on every exit path including
  errors.

Images take the identical path: `image` decodes to RGB, then the same
`StretchDIBits` call. There is one rendering pipeline, not two.

`pdfium.dll` is bundled (BSD-licensed, no copyleft obligations).

### 9.2 `sumatra` (fallback, Windows)

Used only when the GDI path fails with a rendering error, or when no device
context can be created. The fallback is tried **inside the same attempt** — GDI
failing and Sumatra then succeeding counts as one attempt, not two. Sumatra is
never consulted for printer-level errors, since a missing printer is not a
rendering problem.

**Not bundled.** Printy looks for an installed `SumatraPDF.exe` in the standard
install locations and in an optional path configured in Settings. Bundling it
would attach GPLv3 obligations to the installer; calling an independently
installed copy does not. If none is found, the fallback is unavailable and the
job fails with a clear message rather than disappearing silently.

Invocation: `SumatraPDF.exe -print-to "<printer>" -silent -print-settings
"<settings>" "<file>"`. Success is judged by exit code.

### 9.3 `macos_cups` (development platform)

Printer list from `lpstat -p -d`, printing via
`lp -d <printer> -n <copies> -o sides=... -o ColorModel=...`. No rasterisation —
CUPS accepts the PDF and images directly. This keeps the entire application
operable and demonstrable on macOS.

### 9.4 `fake` (tests)

Records every `PrintRequest` in a vector and can be configured to fail with a
chosen error kind. Lets watcher, intake, queue, state machine and post-print
rules be tested end to end without a printer.

## 10. User interface

Sidebar shell taken from tabsy: fixed 240px teal sidebar with logo, wordmark and
navigation; content area on cream.

Navigation: **Ordner**, **Verlauf**, **Einstellungen**, **Über**.

### Ordner (start screen)

Header line with aggregate status ("3 aktiv · 12 heute gedruckt") and the global
pause switch — one click, no menu diving.

One card per folder showing: status dot (mint running / coral error / grey
paused), name, file-type badges, path in monospace, printer and settings summary,
last activity and today's count. Error folders show the reason and, where
applicable, the retry countdown. Card actions: pause, scan now, edit, open in
Explorer/Finder. A dashed "Ordner hinzufügen" tile closes the list.

### Folder dialog

Native folder picker, file-type chips, interval, printer dropdown (system list,
default preselected), copies, duplex, colour, post-print rule. **Controls the
selected printer does not support are disabled, not hidden** — a visibly greyed
duplex switch is honest; a setting that is silently ignored is not.

When a folder is added, everything already in it is marked as *seen*, not
printed. A checkbox — "vorhandene 23 Dateien jetzt mitdrucken" — enables the
opposite explicitly. The reverse default has cost people a paper tray.

### Verlauf

Job list with file, folder, printer, time and outcome. Filterable by state.
Per-row reprint.

### Einstellungen

Autostart on login, start minimised, notification mode, theme, optional
SumatraPDF path, database location.

### Über

Version, licences (generated as in tabsy's `gen-licenses.sh`).

### Tray and background behaviour

Closing the window hides it; the app keeps watching. The tray icon reflects
state: mint when running, coral on error, grey when paused. Context menu: open,
pause/resume, quit. Autostart launches minimised to tray.

## 11. Tauri command surface

`list_printers`, `printer_capabilities`, `list_folders`, `create_folder`,
`update_folder`, `delete_folder`, `set_folder_enabled`, `scan_now`, `list_jobs`,
`reprint_job`, `get_settings`, `update_settings`, `set_global_paused`,
`get_status`, `pick_folder`, `open_path`.

Backend events pushed to the frontend: `job_updated`, `folder_status_changed`,
`queue_status_changed`.

## 12. Operational edge cases

These are requirements, not notes:

- `printed/` and `failed/` are excluded from scanning. Otherwise the watcher
  rediscovers the file it just moved and prints forever.
- Move with a name collision appends a timestamp; it never overwrites. Two scans
  with the same default filename must not consume each other.
- A folder that disappears at runtime sets folder status to `path_missing` and
  stops its watcher task instead of logging an error every tick.
- Poll interval has a floor of 1 second.
- Post-print rule failure (e.g. the file is locked by another process when moving)
  marks the job failed *after* a successful print and does not reprint it.

## 13. Testing strategy

**Rust, against real temp directories and the `fake` backend:**

- Stability check: a growing file is not printed until it stops growing
- Deduplication in `keep` mode, including a byte-identical file under a new name
- Each post-print rule, including move-with-collision
- `printed/` and `failed/` are never rescanned
- State machine transitions and retry backoff timing
- Crash recovery: a job left in `Printing` becomes `Failed`, never reprinted
- Error taxonomy: a file error advances the queue, a printer error pauses it
- Config errors set folder status without repeat notifications

**Frontend:** Vitest + Testing Library, as in tabsy. Folder card states, folder
dialog validation, capability-driven disabling of controls.

**Explicitly not unit-tested:** the GDI code itself. Win32 handles against a real
spooler cannot be mocked honestly, and a mock that passes proves nothing. It is
covered by the manual checklist below instead.

## 14. Manual Windows verification checklist

Run once on a Windows machine per release:

1. Printer list matches the Windows printer list; the default is preselected.
2. A single-page PDF prints, correctly oriented and scaled.
3. A multi-page PDF prints in the right page order.
4. Two copies produce two copies.
5. Duplex produces a double-sided document.
6. Mono mode on a colour printer produces greyscale.
7. Duplex and colour controls are disabled for a printer that lacks them.
8. A JPG and a PNG print through the same path.
9. Switching the printer off mid-queue pauses the queue; switching it back on
   resumes it without losing jobs.
10. A corrupt PDF fails alone and the queue continues.
11. Autostart launches minimised to tray after a reboot.
12. Closing the window keeps watching; quitting via tray stops it.

## 15. Risks and open points

- **GDI output is raster, not vector.** Text is rasterised at up to 300 DPI
  rather than passed through as curves. Acceptable for scans, forms and labels;
  visible on very fine typography. Revisit only if it proves a problem in use.
- **"Done" means the spooler accepted the job, not that paper came out.** Neither
  GDI nor CUPS reports back that a sheet physically emerged. A paper jam after
  the handover is invisible to Printy and the job will read as successful. The UI
  must not claim more than that.
- **A slow interval delays printing by up to two ticks.** The stability check
  needs two consecutive unchanged scans, so a folder set to 30 seconds prints up
  to a minute after the file lands. This is a deliberate trade for not printing
  half-written files; the interval field should make the cost obvious.
- **Windows-only code cannot be verified from macOS.** Roughly 300 lines of GDI
  are the only part gated on a Windows machine; everything else is testable
  locally through the CUPS and fake backends.
- **The project directory is currently `folder_printe`.** Renaming it to `printy`
  before implementation would be tidier but is not required.
