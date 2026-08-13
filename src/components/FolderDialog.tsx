import { useEffect, useRef, useState, type FormEvent, type JSX } from "react";
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
import SearchSelect from "./SearchSelect";

const DUPLEX_OPTIONS: { value: DuplexMode; label: string }[] = [
  { value: "simplex", label: "Einseitig" },
  { value: "long_edge", label: "Duplex (lange Kante)" },
  { value: "short_edge", label: "Duplex (kurze Kante)" },
];

const COLOR_OPTIONS: { value: ColorMode; label: string }[] = [
  { value: "mono", label: "Schwarz-weiß" },
  { value: "color", label: "Farbe" },
];

const POST_ACTION_OPTIONS: { value: PostAction; label: string }[] = [
  { value: "move", label: "In Unterordner verschieben" },
  { value: "keep", label: "Liegen lassen" },
  { value: "delete", label: "Löschen" },
];

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
  /**
   * The `default_poll_interval_secs` setting, used only as the initial value
   * of a newly created folder's own interval. Editing an existing folder
   * never reads this -- its stored interval always wins.
   */
  defaultPollIntervalSecs: number;
  onPrinterChange: (printer: string) => void;
  onCancel: () => void;
  onSubmit: (result: FolderDialogResult) => void;
  saving?: boolean;
}

const UNSUPPORTED = "Dieser Drucker unterstützt das nicht.";

/** Shown when the user tries to leave a form they have already changed. */
const DISCARD_CHANGES_PROMPT =
  "Ungespeicherte Änderungen verwerfen? Die eingegebenen Daten gehen sonst verloren.";

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

function initialState(
  folder: WatchFolder | null,
  printers: PrinterInfo[],
  defaultPollIntervalSecs: number,
): FormState {
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
    pollInterval: String(defaultPollIntervalSecs),
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
  defaultPollIntervalSecs,
  onPrinterChange,
  onCancel,
  onSubmit,
  saving = false,
}: FolderDialogProps): JSX.Element {
  const creating = folder === null;
  const [form, setForm] = useState<FormState>(() =>
    initialState(folder, printers, defaultPollIntervalSecs),
  );
  const [printExisting, setPrintExisting] = useState(false);
  const [existingCount, setExistingCount] = useState<number | null>(null);

  // Captured once on mount so a later edit can be compared against it -- the
  // page's only way back must never throw work away silently.
  const initialFormRef = useRef(form);
  const isDirty =
    printExisting || JSON.stringify(form) !== JSON.stringify(initialFormRef.current);

  function handleCancel(): void {
    if (isDirty && !window.confirm(DISCARD_CHANGES_PROMPT)) return;
    onCancel();
  }

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
    <section className="screen">
      <button type="button" className="link-btn back-link" onClick={handleCancel}>
        ← Zurück
      </button>
      <h1>{creating ? "Ordner hinzufügen" : "Ordner bearbeiten"}</h1>
      <form className="card" onSubmit={handleSubmit}>
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
          <SearchSelect<string>
            id="fd-printer"
            value={form.printerName}
            onChange={(printerName) => {
              setForm((f) => ({ ...f, printerName }));
              onPrinterChange(printerName);
            }}
            options={printers.map((p) => ({
              value: p.name,
              label: p.is_default ? `${p.name} (Standard)` : p.name,
            }))}
          />
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
            <SearchSelect<DuplexMode>
              id="fd-duplex"
              disabled={!canDuplex}
              value={form.duplex}
              onChange={(duplex) => setForm((f) => ({ ...f, duplex }))}
              options={DUPLEX_OPTIONS}
            />
            {!canDuplex && <p className="helper">{UNSUPPORTED}</p>}
          </div>
        </div>

        <div className="row">
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-color">Farbe</label>
            <SearchSelect<ColorMode>
              id="fd-color"
              disabled={!canColor}
              value={form.colorMode}
              onChange={(colorMode) => setForm((f) => ({ ...f, colorMode }))}
              options={COLOR_OPTIONS}
            />
            {!canColor && <p className="helper">{UNSUPPORTED}</p>}
          </div>

          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="fd-post">Nach dem Druck</label>
            <SearchSelect<PostAction>
              id="fd-post"
              value={form.postAction}
              onChange={(postAction) => setForm((f) => ({ ...f, postAction }))}
              options={POST_ACTION_OPTIONS}
            />
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
          <button type="button" className="link-btn" onClick={handleCancel}>
            Abbrechen
          </button>
          <button type="submit" className="btn" disabled={!valid || saving}>
            {saving ? "Speichere …" : creating ? "Anlegen" : "Speichern"}
          </button>
        </div>
      </form>
    </section>
  );
}
