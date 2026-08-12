import type { JSX } from "react";
import type { WatchFolder } from "../lib/types";
import {
  countdownSeconds,
  fileTypeLabels,
  folderStatusKind,
  folderStatusLabel,
  formatDateTime,
  parseFileTypes,
  settingsSummary,
  type FolderActivity,
} from "../lib/format";

export interface FolderCardProps {
  folder: WatchFolder;
  activity: FolderActivity;
  /** True while the whole queue waits for a printer to come back. */
  queueHeld: boolean;
  /** Injected rather than read from the clock, so the countdown is testable. */
  nowMs: number;
  busy?: boolean;
  onToggleEnabled: (folder: WatchFolder) => void;
  onScanNow: (folder: WatchFolder) => void;
  onEdit: (folder: WatchFolder) => void;
  onReveal: (folder: WatchFolder) => void;
}

/**
 * One watched folder at a glance. Presentational only — every action is handed
 * back to the Ordner screen, which owns the mutations.
 */
export default function FolderCard({
  folder,
  activity,
  queueHeld,
  nowMs,
  busy = false,
  onToggleEnabled,
  onScanNow,
  onEdit,
  onReveal,
}: FolderCardProps): JSX.Element {
  const kind = folderStatusKind(folder, queueHeld);
  const enabled = folder.enabled !== 0;
  const labels = fileTypeLabels(parseFileTypes(folder.file_types));
  const countdown = countdownSeconds(activity.nextAttemptAt, nowMs);

  return (
    <li className={`folder-card${kind === "error" ? " has-error" : ""}`}>
      <div className="folder-head">
        <span
          className={`status-dot ${kind}`}
          data-testid="status-dot"
          aria-hidden="true"
        />
        <span className="folder-name">{folder.name}</span>
        <span className="folder-state">{folderStatusLabel(kind, folder)}</span>
        <span className="chip-row" data-testid="type-badges">
          {labels.map((label) => (
            <span key={label} className="badge">
              {label}
            </span>
          ))}
        </span>
      </div>

      <span className="mono muted" data-testid="folder-path">
        {folder.path}
      </span>

      <span className="folder-summary">{settingsSummary(folder)}</span>

      {kind === "error" && (
        <span className="folder-error">
          {folder.status === "path_missing"
            ? "Der Ordner existiert nicht mehr. Die Überwachung ist gestoppt."
            : "Der eingestellte Drucker ist nicht mehr installiert."}
        </span>
      )}

      {countdown !== null && (
        <span className="folder-error" data-testid="retry-countdown">
          Nächster Versuch in {countdown} s
        </span>
      )}

      <div className="folder-meta">
        <span>{activity.printedToday} heute gedruckt</span>
        <span data-testid="last-activity">
          Zuletzt: {formatDateTime(activity.lastActivity)}
        </span>
        {activity.failedCount > 0 && (
          <span>{activity.failedCount} fehlgeschlagen</span>
        )}
      </div>

      <div className="folder-actions">
        <button
          type="button"
          className="link-btn"
          disabled={busy}
          onClick={() => onToggleEnabled(folder)}
        >
          {enabled ? "Pausieren" : "Fortsetzen"}
        </button>
        <button
          type="button"
          className="link-btn"
          disabled={busy}
          onClick={() => onScanNow(folder)}
        >
          Jetzt scannen
        </button>
        <button type="button" className="link-btn" onClick={() => onEdit(folder)}>
          Bearbeiten
        </button>
        <button type="button" className="link-btn" onClick={() => onReveal(folder)}>
          Im Explorer öffnen
        </button>
      </div>
    </li>
  );
}
