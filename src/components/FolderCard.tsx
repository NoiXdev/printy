import { useEffect, useState, type JSX, type MouseEvent } from "react";
import type { WatchFolder } from "../lib/types";
import { closeExclusive, openExclusive } from "../lib/menuCoordinator";
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
import FolderMenu, { type FolderMenuItem } from "./FolderMenu";

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
  onDelete: (folder: WatchFolder) => void;
}

/**
 * One watched folder at a glance. Presentational only — every action is handed
 * back to the Ordner screen, which owns the mutations.
 *
 * Every action lives in one menu, reachable two ways: the visible "..."
 * button (keyboard- and touch-reachable) and a right-click anywhere on the
 * card. Only one card's menu is ever open at once -- see `menuCoordinator`.
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
  onDelete,
}: FolderCardProps): JSX.Element {
  const kind = folderStatusKind(folder, queueHeld);
  const enabled = folder.enabled !== 0;
  const labels = fileTypeLabels(parseFileTypes(folder.file_types));
  const countdown = countdownSeconds(activity.nextAttemptAt, nowMs);
  const [menuOpen, setMenuOpen] = useState(false);

  useEffect(() => {
    if (!menuOpen) return;
    const closeSelf = (): void => setMenuOpen(false);
    openExclusive(closeSelf);
    return () => closeExclusive(closeSelf);
  }, [menuOpen]);

  function handleContextMenu(e: MouseEvent<HTMLLIElement>): void {
    e.preventDefault();
    setMenuOpen(true);
  }

  const menuItems: FolderMenuItem[] = [
    {
      key: "toggle",
      label: enabled ? "Pausieren" : "Fortsetzen",
      onSelect: () => onToggleEnabled(folder),
      disabled: busy,
    },
    {
      key: "scan",
      label: "Jetzt scannen",
      onSelect: () => onScanNow(folder),
      disabled: busy,
    },
    { key: "edit", label: "Bearbeiten", onSelect: () => onEdit(folder) },
    { key: "reveal", label: "Im Explorer öffnen", onSelect: () => onReveal(folder) },
    {
      key: "delete",
      label: "Löschen",
      onSelect: () => onDelete(folder),
      destructive: true,
      separated: true,
    },
  ];

  return (
    <li
      className={`folder-card${kind === "error" ? " has-error" : ""}`}
      onContextMenu={handleContextMenu}
    >
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
        <FolderMenu
          label={`Aktionen für ${folder.name}`}
          items={menuItems}
          open={menuOpen}
          onOpenChange={setMenuOpen}
        />
      </div>
    </li>
  );
}
