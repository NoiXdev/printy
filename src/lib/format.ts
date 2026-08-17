import type { DisabledReason, FolderDeleteImpact, JobState, PrintJob, WatchFolder } from "./types";

export interface FileTypeChip {
  id: string;
  label: string;
  /** Every extension this chip stands for, lowercase and without a dot. */
  extensions: string[];
}

/** The four formats the print pipeline supports (spec section 2). */
export const FILE_TYPE_CHIPS: ReadonlyArray<FileTypeChip> = [
  { id: "pdf", label: "PDF", extensions: ["pdf"] },
  { id: "jpg", label: "JPG", extensions: ["jpg", "jpeg"] },
  { id: "png", label: "PNG", extensions: ["png"] },
  { id: "tiff", label: "TIFF", extensions: ["tif", "tiff"] },
];

/**
 * SQLite's `datetime('now')` writes `YYYY-MM-DD HH:MM:SS` in UTC. Handing that
 * straight to `new Date` makes V8 read it as *local* time, which shifts every
 * timestamp by the timezone offset. Normalise explicitly.
 */
export function parseDbTimestamp(value: string | null): Date | null {
  if (value === null || value.trim() === "") return null;
  const normalized = value.includes("T") ? value : `${value.replace(" ", "T")}Z`;
  const d = new Date(normalized);
  return Number.isNaN(d.getTime()) ? null : d;
}

export function formatDateTime(value: string | null): string {
  const d = parseDbTimestamp(value);
  if (d === null) return "—";
  return d.toLocaleString("de-DE", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** The `file_types` column is JSON; a corrupted value must not blank the screen. */
export function parseFileTypes(json: string): string[] {
  try {
    const parsed: unknown = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((v): v is string => typeof v === "string");
  } catch {
    return [];
  }
}

export function chipIdsFromFileTypes(fileTypes: string[]): string[] {
  const set = new Set(fileTypes.map((t) => t.toLowerCase()));
  return FILE_TYPE_CHIPS.filter((c) => c.extensions.some((e) => set.has(e))).map((c) => c.id);
}

export function fileTypesFromChipIds(chipIds: string[]): string[] {
  return FILE_TYPE_CHIPS.filter((c) => chipIds.includes(c.id)).flatMap((c) => c.extensions);
}

export function fileTypeLabels(fileTypes: string[]): string[] {
  return chipIdsFromFileTypes(fileTypes).map(
    (id) => FILE_TYPE_CHIPS.find((c) => c.id === id)?.label ?? id.toUpperCase(),
  );
}

export type FolderStatusKind = "running" | "error" | "paused" | "waiting";

/**
 * The user's per-folder pause wins over everything: a folder the user switched
 * off must not blink amber because some other printer is unreachable. A config
 * error outranks the printer hold, because it needs the user rather than time.
 */
export function folderStatusKind(folder: WatchFolder, queueHeld: boolean): FolderStatusKind {
  if (folder.enabled === 0) return "paused";
  if (folder.status !== "ok") return "error";
  if (queueHeld) return "waiting";
  return "running";
}

export function folderStatusLabel(kind: FolderStatusKind, folder: WatchFolder): string {
  switch (kind) {
    case "running":
      return "Aktiv";
    case "paused":
      return "Pausiert";
    case "waiting":
      return "Wartet auf Drucker";
    case "error":
      return folder.status === "path_missing"
        ? "Ordner nicht gefunden"
        : "Drucker nicht mehr installiert";
  }
}

const DUPLEX_LABEL: Record<WatchFolder["duplex"], string> = {
  simplex: "Einseitig",
  long_edge: "Duplex (lange Kante)",
  short_edge: "Duplex (kurze Kante)",
};

const COLOR_LABEL: Record<WatchFolder["color_mode"], string> = {
  color: "Farbe",
  mono: "Schwarz-weiß",
};

const POST_ACTION_LABEL: Record<WatchFolder["post_action"], string> = {
  move: "verschieben",
  keep: "liegen lassen",
  delete: "löschen",
};

export function settingsSummary(folder: WatchFolder): string {
  const copies = folder.copies === 1 ? "1 Kopie" : `${folder.copies} Kopien`;
  return [
    folder.printer_name,
    copies,
    DUPLEX_LABEL[folder.duplex],
    COLOR_LABEL[folder.color_mode],
    POST_ACTION_LABEL[folder.post_action],
  ].join(" · ");
}

/**
 * The database cascades a folder delete onto every one of its jobs. This
 * builds the confirmation prompt's body: it names the two counts concretely
 * (never a bare "0 wartende Aufträge" when the honest statement is that there
 * simply is nothing) and always reassures that the watched folder's own files
 * are untouched -- the reassurance a cautious user needs before they click.
 */
export function folderDeleteWarning(impact: FolderDeleteImpact): string {
  const { waiting, history } = impact;
  const untouched =
    "Die Dateien im überwachten Ordner selbst werden dabei nicht angetastet.";

  if (waiting === 0 && history === 0) {
    return (
      "Für diesen Ordner gibt es weder wartende Aufträge noch Einträge im Verlauf " +
      `– es wird nichts mitgelöscht. ${untouched}`
    );
  }

  const waitingText = waiting === 1 ? "1 wartender Auftrag" : `${waiting} wartende Aufträge`;
  const historyText = history === 1 ? "1 Eintrag im Verlauf" : `${history} Einträge im Verlauf`;

  if (waiting === 0) {
    return `Keine wartenden Aufträge, aber ${historyText} werden mitgelöscht. ${untouched}`;
  }
  if (history === 0) {
    return `${waitingText} werden mitgelöscht, aber kein Eintrag im Verlauf. ${untouched}`;
  }
  return `${waitingText} und ${historyText} werden mitgelöscht. ${untouched}`;
}

export interface FolderActivity {
  printedToday: number;
  /** Raw database timestamp of the newest finished job, or null. */
  lastActivity: string | null;
  /** Epoch ms of the soonest pending retry in this folder, or null. */
  nextAttemptAt: number | null;
  failedCount: number;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

export function folderActivity(
  jobs: PrintJob[],
  folderId: number,
  nowMs: number,
): FolderActivity {
  const today = new Date(nowMs);
  const mine = jobs.filter((j) => j.folder_id === folderId);

  let printedToday = 0;
  let failedCount = 0;
  let lastActivity: string | null = null;
  let lastMs = -1;
  let nextAttemptAt: number | null = null;

  for (const j of mine) {
    if (j.state === "failed") failedCount += 1;
    if (j.state === "retrying" && j.next_attempt_at !== null) {
      if (nextAttemptAt === null || j.next_attempt_at < nextAttemptAt) {
        nextAttemptAt = j.next_attempt_at;
      }
    }
    const finished = parseDbTimestamp(j.finished_at);
    if (finished === null) continue;
    if (j.state === "done" && isSameDay(finished, today)) printedToday += 1;
    if (finished.getTime() > lastMs) {
      lastMs = finished.getTime();
      lastActivity = j.finished_at;
    }
  }

  return { printedToday, lastActivity, nextAttemptAt, failedCount };
}

export function countdownSeconds(nextAttemptAt: number | null, nowMs: number): number | null {
  if (nextAttemptAt === null) return null;
  return Math.max(0, Math.ceil((nextAttemptAt - nowMs) / 1000));
}

/**
 * The stability check needs two consecutive unchanged scans, so a file lands on
 * paper up to two intervals after it appears. The interval field has to say so —
 * a 30-second interval quietly costing a minute is the kind of surprise that
 * makes people distrust the app.
 */
export function stabilityHint(intervalSecs: number): string {
  const worst = Math.max(1, intervalSecs) * 2;
  const amount =
    worst >= 120
      ? `${Math.round(worst / 60)} Minuten`
      : worst === 60
        ? "1 Minute"
        : `${worst} Sekunden`;
  return (
    `Eine Datei gilt erst als fertig, wenn sie sich zwei Scans lang nicht mehr ändert. ` +
    `Neue Dateien werden deshalb bis zu ${amount} nach dem Auftauchen gedruckt.`
  );
}

const JOB_STATE_LABEL: Record<JobState, string> = {
  queued: "Wartet",
  printing: "Druckt",
  retrying: "Wiederholt",
  done: "Gedruckt",
  failed: "Fehlgeschlagen",
};

export function jobOutcomeLabel(state: JobState): string {
  return JOB_STATE_LABEL[state];
}

/** Names the concrete reason `import_config_cmd` force-disabled a folder. */
export function disabledReasonText(reason: DisabledReason): string {
  if (reason.path_missing && reason.printer_missing) {
    return "Ordnerpfad nicht gefunden und Drucker nicht installiert.";
  }
  if (reason.path_missing) return "Ordnerpfad auf diesem Rechner nicht gefunden.";
  return "Drucker auf diesem Rechner nicht installiert.";
}

/** Aggregate impact of "Alles ersetzen": every existing folder, and what it takes with it. */
export interface ReplaceAllImpact {
  folders: number;
  waiting: number;
  history: number;
}

/**
 * Builds the destructive confirmation for replace-mode import, with the same
 * honesty as `folderDeleteWarning`: concrete counts, never a bare zero, and
 * an explicit reassurance that the watched folders' own files are untouched.
 */
export function replaceAllWarning(impact: ReplaceAllImpact): string {
  const { folders, waiting, history } = impact;
  const untouched =
    "Die Dateien in den überwachten Ordnern selbst werden dabei nicht angetastet.";

  if (folders === 0) {
    return (
      "Es gibt noch keinen bestehenden Ordner – „Alles ersetzen“ hat daher keine Auswirkung " +
      `auf vorhandene Daten. ${untouched}`
    );
  }

  const folderText =
    folders === 1 ? "1 bestehender Ordner wird" : `${folders} bestehende Ordner werden`;
  const waitingText = waiting === 1 ? "1 wartender Auftrag" : `${waiting} wartende Aufträge`;
  const historyText = history === 1 ? "1 Eintrag im Verlauf" : `${history} Einträge im Verlauf`;
  const replaced = `${folderText} gelöscht und durch die Ordner aus der Datei ersetzt`;

  if (waiting === 0 && history === 0) {
    return `${replaced}, ohne wartende Aufträge oder Einträge im Verlauf mitzunehmen. ${untouched}`;
  }
  if (waiting === 0) {
    return `${replaced}, mitsamt ${historyText}, aber ohne wartende Aufträge. ${untouched}`;
  }
  if (history === 0) {
    return `${replaced}, mitsamt ${waitingText}, aber ohne Einträge im Verlauf. ${untouched}`;
  }
  return `${replaced}, mitsamt ${waitingText} und ${historyText}. ${untouched}`;
}
