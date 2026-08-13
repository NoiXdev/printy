import { useEffect, useId, useRef, type JSX, type KeyboardEvent, type MouseEvent } from "react";

export interface ConfirmDialogProps {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  /** Styles the confirm button as a destructive action (e.g. red, not coral). */
  destructive?: boolean;
  /** Disables both buttons, e.g. while the confirmed action is in flight. */
  pending?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * A small in-app confirmation overlay. Used instead of the native
 * `window.confirm`/Tauri dialog plugin because a destructive confirmation
 * needs control neither of those give: a button that visibly looks
 * dangerous, and a focus that deliberately starts on Cancel rather than on
 * the action that cannot be undone.
 */
export default function ConfirmDialog({
  title,
  message,
  confirmLabel,
  cancelLabel = "Abbrechen",
  destructive = false,
  pending = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps): JSX.Element {
  const titleId = useId();
  const messageId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);

  // The destructive action must never be where focus lands by default --
  // a stray Enter press must not delete anything.
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  function handleKeyDown(e: KeyboardEvent<HTMLDivElement>): void {
    if (e.key === "Escape") {
      e.stopPropagation();
      onCancel();
    }
  }

  function handleBackdropMouseDown(e: MouseEvent<HTMLDivElement>): void {
    if (e.target === e.currentTarget) onCancel();
  }

  return (
    <div className="dialog-backdrop" onMouseDown={handleBackdropMouseDown}>
      <div
        className="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={messageId}
        onKeyDown={handleKeyDown}
      >
        <h2 id={titleId}>{title}</h2>
        <p id={messageId}>{message}</p>
        <div className="modal-actions">
          <button
            type="button"
            className="link-btn"
            ref={cancelRef}
            disabled={pending}
            onClick={onCancel}
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            className={`btn${destructive ? " btn-danger" : ""}`}
            disabled={pending}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
