import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom";
import ConfirmDialog from "./ConfirmDialog";

function renderDialog(overrides: Partial<Parameters<typeof ConfirmDialog>[0]> = {}) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  render(
    <ConfirmDialog
      title="Ordner löschen?"
      message="3 wartende Aufträge und 47 Einträge im Verlauf werden mitgelöscht."
      confirmLabel="Endgültig löschen"
      destructive
      onConfirm={onConfirm}
      onCancel={onCancel}
      {...overrides}
    />,
  );
  return { onConfirm, onCancel };
}

describe("ConfirmDialog", () => {
  it("renders as an alertdialog naming the title and message", () => {
    renderDialog();
    const dialog = screen.getByRole("alertdialog");
    expect(dialog).toHaveAccessibleName("Ordner löschen?");
    expect(dialog).toHaveAccessibleDescription(
      "3 wartende Aufträge und 47 Einträge im Verlauf werden mitgelöscht.",
    );
  });

  it("focuses Cancel by default, never the destructive confirm button", () => {
    renderDialog();
    expect(screen.getByRole("button", { name: "Abbrechen" })).toHaveFocus();
  });

  it("styles the confirm button as destructive when asked", () => {
    renderDialog({ destructive: true });
    expect(screen.getByRole("button", { name: "Endgültig löschen" })).toHaveClass("btn-danger");
  });

  it("does not style the confirm button as destructive by default", () => {
    renderDialog({ destructive: false });
    expect(screen.getByRole("button", { name: "Endgültig löschen" })).not.toHaveClass(
      "btn-danger",
    );
  });

  it("calls onConfirm when the confirm button is clicked", () => {
    const { onConfirm } = renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Endgültig löschen" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when the cancel button is clicked", () => {
    const { onCancel } = renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel on Escape", () => {
    const { onCancel } = renderDialog();
    fireEvent.keyDown(screen.getByRole("alertdialog"), { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when clicking the backdrop but not when clicking inside the dialog", () => {
    const { onCancel } = renderDialog();
    fireEvent.mouseDown(screen.getByRole("alertdialog"));
    expect(onCancel).not.toHaveBeenCalled();

    // The backdrop is the alertdialog's parent.
    fireEvent.mouseDown(screen.getByRole("alertdialog").parentElement as HTMLElement);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("disables both buttons while pending", () => {
    renderDialog({ pending: true });
    expect(screen.getByRole("button", { name: "Abbrechen" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Endgültig löschen" })).toBeDisabled();
  });
});
