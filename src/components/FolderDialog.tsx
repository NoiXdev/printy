import { useEffect, useState, type FormEvent, type JSX } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ColorMode,
  DuplexMode,
  NewFolder,
  PostAction,
  PrinterCapabilities,
  PrinterInfo,
  WatchFolder,
} from "../lib/types";
import {
  FILE_TYPE_CHIPS,
  chipIdsFromFileTypes,
  fileTypesFromChipIds,
  parseFileTypes,
  stabilityHint,
} from "../lib/format";

export interface FolderDialogResult {
  folder: NewFolder;
  printExisting: boolean;
}

export interface FolderDialogProps {
  /** null creates a folder; a row edits it. */
  folder: WatchFolder | null;
  printers: PrinterInfo[];
  /** null while the capability probe is still running. */
  capabilities: PrinterCapabilities | null;
  /** Injected so the dialog stays free of query wiring and easy to test. */
  countExisting: (path: string, fileTypes: string[]) => Promise<number>;
  onPrinterChange: (printer: string) => void;
  onCancel: () => void;
  onSubmit: (result: FolderDialogResult) => void;
  saving?: boolean;
}

const UNSUPPORTED = "Dieser Drucker unterstützt das nicht.";

interface FormState {
  name: string;
  path: string;
  pollInterval: string;
  chipIds: string[];
  printerName: string;
  copies: string;
  duplex: DuplexMode;
  colorMode: ColorMode;
  postAction: PostAction;
  fitToPage: boolean;
}

function initialState(folder: WatchFolder | null, printers: PrinterInfo[]): FormState {
  if (folder !== null) {
    return {
      name: folder.name,
      path: folder.path,
      pollInterval: String(folder.poll_interval_secs),
      chipIds: chipIdsFromFileTypes(parseFileTypes(folder.file_types)),
      printerName: folder.printer_name,
      copies: String(folder.copies),
      duplex: folder.duplex,
      colorMode: folder.color_mode,
      postAction: folder.post_action,
      fitToPage: folder.fit_to_page !== 0,
    };
  }
  const preselected =
    printers.find((p) => p.is_default)?.name ?? printers[0]?.name ?? "";
  return {
    name: "",
    path: "",
    pollInterval: "5",
    chipIds: ["pdf"],
    printerName: preselected,
    copies: "1",
    duplex: "simplex",
    colorMode: "mono",
    postAction: "move",
    // A folder scales its contents to the page unless the user says otherwise.
    fitToPage: true,
  };
}

export default function FolderDialog({
  folder,
  printers,
  capabilities,
  countExisting,
  onPrinterChange,
  onCancel,
  onSubmit,
  saving = false,
}: FolderDialogProps): JSX.Element {
  const creating = folder === null;
  const [form, setForm] = useState<FormState>(() => initialState(folder, printers));
  const [printExisting, setPrintExisting] = useState(false);
  const [existingCount, setExistingCount] = useState<number | null>(null);

  const fileTypes = fileTypesFromChipIds(form.chipIds);
  const intervalSecs = Math.max(1, Number.parseInt(form.pollInterval, 10) || 1);

  // Only meaningful while creating: an existing folder has long since been
  // catalogued, and re-offering to print its contents would be a paper trap.
  // Fires even before a path is chosen — the count simply tracks whatever the
  // form currently holds, so it is never stale once the user does pick one.
  useEffect(() => {
    if (!creating || fileTypes.length === 0) {
      setExistingCount(null);
      return;
    }
    let cancelled = false;
    void countExisting(form.path.trim(), fileTypes)
      .then((n) => {
        if (!cancelled) setExistingCount(n);
      })
      .catch(() => {
        if (!cancelled) setExistingCount(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [creating, form.path, form.chipIds.join(",")]);

  // Capabilities are unknown while the probe runs; assume nothing is blocked
  // rather than greying out a control the printer may well support.
  const canDuplex = capabilities?.duplex ?? true;
  const canColor = capabilities?.color ?? true;
  const canCopies = capabilities?.copies ?? true;

  const valid =
    form.name.trim() !== "" && form.path.trim() !== "" && fileTypes.length > 0;

  function toggleChip(id: string): void {
    setForm((f) => ({
      ...f,
      chipIds: f.chipIds.includes(id)
        ? f.chipIds.filter((c) => c !== id)
        : [...f.chipIds, id],
    }));
  }

  async function browse(): Promise<void> {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") {
      setForm((f) => ({ ...f, path: picked }));
    }
  }

  function handleSubmit(e: FormEvent<HTMLFormElement>): void {
    e.preventDefault();
    if (!valid) return;
    onSubmit({
      folder: {
        name: form.name.trim(),
        path: form.path.trim(),
        poll_interval_secs: intervalSecs,
        file_types: fileTypes,
        printer_name: form.printerName,
        // A printer that cannot honour a setting is sent the neutral value, so
        // the stored configuration never claims something that will be ignored.
        copies: canCopies ? Math.max(1, Number.parseInt(form.copies, 10) || 1) : 1,
        duplex: canDuplex ? form.duplex : "simplex",
        color_mode: canColor ? form.colorMode : "mono",
        post_action: form.postAction,
        fit_to_page: form.fitToPage,
      },
      printExisting: creating ? printExisting : false,
    });
  }

  const existingLabel =
    existingCount === null
      ? "Vorhandene Dateien jetzt mitdrucken"
      : `Vorhandene ${existingCount} Dateien jetzt mitdrucken`;

  return (
    <div className="modal-overlay" role="dialog" aria-modal="true" aria-label={
      creating ? "Ordner hinzufügen" : "Ordner bearbeiten"
    }>
      <form className="modal" onSubmit={handleSubmit}>
        <div className="modal-head">
          <h2>{creating ? "Ordner hinzufügen" : "Ordner bearbeiten"}</h2>
        </div>

        <div className="field">
          <label htmlFor="fd-name">Name</label>
          <input
            id="fd-name"
            className="input"
            value={form.name}
            placeholder="z. B. Scanner"
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
          />
        </div>

        <div className="field">
          <label htmlFor="fd-path">Ordner</label>
          <div className="row">
            <input
              id="fd-path"
              className="input"
              style={{ flex: 1 }}
              value={form.path}
              placeholder="C:\\Scans"
              onChange={(e) => setForm((f) => ({ ...f, path: e.target.value }))}
            />
            <button type="button" className="btn btn-quiet" onClick={() => void browse()}>
              Durchsuchen …
            </button>
          </div>
        </div>

        <div className="field">
          <label>Dateitypen</label>
          <div className="chip-row">
            {FILE_TYPE_CHIPS.map((chip) => (
              <button
                key={chip.id}
                type="button"
                className={`chip${form.chipIds.includes(chip.id) ? " on" : ""}`}
                aria-pressed={form.chipIds.includes(chip.id)}
                onClick={() => toggleChip(chip.id)}
              >
                {chip.label}
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <label htmlFor="fd-interval">Prüfintervall (Sekunden)</label>
          <input
            id="fd-interval"
            className="input"
            type="number"
            min={1}
            value={form.pollInterval}
            onChange={(e) => setForm((f) => ({ ...f, pollInterval: e.target.value }))}
          />
          <p className="helper" data-testid="interval-hint">
            {stabilityHint(intervalSecs)}
          </p>
        </div>

        <div className="field">
          <label htmlFor="fd-printer">Drucker</label>
          <select
            id="fd-printer"
            className="input"
            value={form.printerName}
            onChange={(e) => {
              setForm((f) => ({ ...f, printerName: e.target.value }));
              onPrinterChange(e.target.value);
            }}
          >
            {printers.map((p) => (
              <option key={p.name} value={p.name}>
                {p.is_default ? `${p.name} (Standard)` : p.name}
              </option>
            ))}
          </select>
        </div>

        <div className="row">
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-copies">Kopien</label>
            <input
              id="fd-copies"
              className="input"
              type="number"
              min={1}
              disabled={!canCopies}
              value={form.copies}
              onChange={(e) => setForm((f) => ({ ...f, copies: e.target.value }))}
            />
            {!canCopies && <p className="helper">{UNSUPPORTED}</p>}
          </div>

          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-duplex">Duplex</label>
            <select
              id="fd-duplex"
              className="input"
              disabled={!canDuplex}
              value={form.duplex}
              onChange={(e) =>
                setForm((f) => ({ ...f, duplex: e.target.value as DuplexMode }))
              }
            >
              <option value="simplex">Einseitig</option>
              <option value="long_edge">Duplex (lange Kante)</option>
              <option value="short_edge">Duplex (kurze Kante)</option>
            </select>
            {!canDuplex && <p className="helper">{UNSUPPORTED}</p>}
          </div>
        </div>

        <div className="row">
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-color">Farbe</label>
            <select
              id="fd-color"
              className="input"
              disabled={!canColor}
              value={form.colorMode}
              onChange={(e) =>
                setForm((f) => ({ ...f, colorMode: e.target.value as ColorMode }))
              }
            >
              <option value="mono">Schwarz-weiß</option>
              <option value="color">Farbe</option>
            </select>
            {!canColor && <p className="helper">{UNSUPPORTED}</p>}
          </div>

          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-post">Nach dem Druck</label>
            <select
              id="fd-post"
              className="input"
              value={form.postAction}
              onChange={(e) =>
                setForm((f) => ({ ...f, postAction: e.target.value as PostAction }))
              }
            >
              <option value="move">In Unterordner verschieben</option>
              <option value="keep">Liegen lassen</option>
              <option value="delete">Löschen</option>
            </select>
          </div>
        </div>

        <div className="field">
          <label className="switch-label" htmlFor="fd-fit-to-page">
            <input
              id="fd-fit-to-page"
              type="checkbox"
              checked={form.fitToPage}
              onChange={(e) => setForm((f) => ({ ...f, fitToPage: e.target.checked }))}
            />
            <span>Inhalt an Seite anpassen</span>
          </label>
          <p className="helper" data-testid="fit-to-page-hint">
            {form.fitToPage
              ? "Der Inhalt wird auf die Seitengröße skaliert."
              : "Der Inhalt wird in Originalgröße gedruckt."}{" "}
            Zu große Inhalte werden in jedem Fall verkleinert.
          </p>
        </div>

        {creating && (
          <div className="field">
            <label className="switch-label" htmlFor="fd-print-existing">
              <input
                id="fd-print-existing"
                type="checkbox"
                checked={printExisting}
                onChange={(e) => setPrintExisting(e.target.checked)}
              />
              <span>{existingLabel}</span>
            </label>
            <p className="helper">
              Standardmäßig gilt alles, was schon im Ordner liegt, als erledigt. Nur
              neu hinzukommende Dateien werden gedruckt.
            </p>
          </div>
        )}

        <div className="modal-actions">
          <button type="button" className="link-btn" onClick={onCancel}>
            Abbrechen
          </button>
          <button type="submit" className="btn" disabled={!valid || saving}>
            {saving ? "Speichere …" : creating ? "Anlegen" : "Speichern"}
          </button>
        </div>
      </form>
    </div>
  );
}
