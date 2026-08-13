import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@testing-library/jest-dom";
import type { ComponentProps } from "react";
import FolderDialog from "./FolderDialog";
import type { PrinterCapabilities, PrinterInfo, WatchFolder } from "../lib/types";

const openMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: (...a: unknown[]) => openMock(...a) }));

const PRINTERS: PrinterInfo[] = [
  { name: "Brother MFC", is_default: false },
  { name: "HP LaserJet", is_default: true },
];

const ALL_CAPS: PrinterCapabilities = { duplex: true, color: true, copies: true };
const NO_CAPS: PrinterCapabilities = { duplex: false, color: false, copies: false };

function existing(folder: Partial<WatchFolder> = {}): WatchFolder {
  return {
    id: 4,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 30,
    file_types: '["pdf"]',
    printer_name: "Brother MFC",
    copies: 3,
    duplex: "long_edge",
    color_mode: "color",
    post_action: "keep",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    fit_to_page: 1,
    ...folder,
  };
}

function renderDialog(props: Partial<ComponentProps<typeof FolderDialog>> = {}) {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  const onPrinterChange = vi.fn();
  render(
    <FolderDialog
      folder={null}
      printers={PRINTERS}
      capabilities={ALL_CAPS}
      countExisting={async () => 23}
      defaultPollIntervalSecs={5}
      onPrinterChange={onPrinterChange}
      onCancel={onCancel}
      onSubmit={onSubmit}
      {...props}
    />,
  );
  return { onSubmit, onCancel, onPrinterChange };
}

describe("FolderDialog printer capabilities", () => {
  it("disables unsupported controls instead of hiding them", () => {
    renderDialog({ capabilities: NO_CAPS });

    const duplex = screen.getByLabelText("Duplex");
    const color = screen.getByLabelText("Farbe");
    const copies = screen.getByLabelText("Kopien");

    expect(duplex).toBeInTheDocument();
    expect(duplex).toBeDisabled();
    expect(color).toBeDisabled();
    expect(copies).toBeDisabled();

    expect(
      screen.getAllByText("Dieser Drucker unterstützt das nicht.").length,
    ).toBe(3);
  });

  it("leaves supported controls enabled", () => {
    renderDialog({ capabilities: ALL_CAPS });
    expect(screen.getByLabelText("Duplex")).toBeEnabled();
    expect(screen.getByLabelText("Farbe")).toBeEnabled();
    expect(screen.getByLabelText("Kopien")).toBeEnabled();
    expect(screen.queryByText("Dieser Drucker unterstützt das nicht.")).not.toBeInTheDocument();
  });

  it("preselects the system default printer when creating", () => {
    renderDialog();
    expect(screen.getByLabelText("Drucker")).toHaveTextContent("HP LaserJet (Standard)");
  });

  it("keeps the folder's own printer when editing", () => {
    renderDialog({ folder: existing() });
    expect(screen.getByLabelText("Drucker")).toHaveTextContent("Brother MFC");
  });

  it("asks the parent to refetch capabilities when the printer changes", () => {
    const { onPrinterChange } = renderDialog();
    fireEvent.click(screen.getByLabelText("Drucker"));
    fireEvent.mouseDown(screen.getByText("Brother MFC"));
    expect(onPrinterChange).toHaveBeenCalledWith("Brother MFC");
  });
});

describe("FolderDialog print-existing checkbox", () => {
  it("defaults to off and names the number of files found", async () => {
    renderDialog();
    const box = screen.getByLabelText(/jetzt mitdrucken/);
    expect(box).not.toBeChecked();
    await waitFor(() =>
      expect(screen.getByLabelText("Vorhandene 23 Dateien jetzt mitdrucken")).toBeInTheDocument(),
    );
  });

  it("submits print_existing false unless the user ticks it", async () => {
    const { onSubmit } = renderDialog();
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    fireEvent.change(screen.getByLabelText("Ordner"), {
      target: { value: "/Users/tim/Scans" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Anlegen" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].printExisting).toBe(false);
    expect(onSubmit.mock.calls[0][0].folder).toEqual({
      name: "Scanner",
      path: "/Users/tim/Scans",
      poll_interval_secs: 5,
      file_types: ["pdf"],
      printer_name: "HP LaserJet",
      copies: 1,
      duplex: "simplex",
      color_mode: "mono",
      post_action: "move",
      fit_to_page: true,
    });
  });

  it("submits print_existing true once the user opts in", async () => {
    const { onSubmit } = renderDialog();
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    fireEvent.change(screen.getByLabelText("Ordner"), {
      target: { value: "/Users/tim/Scans" },
    });
    fireEvent.click(screen.getByLabelText(/jetzt mitdrucken/));
    fireEvent.click(screen.getByRole("button", { name: "Anlegen" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].printExisting).toBe(true);
  });

  it("does not offer the checkbox when editing an existing folder", () => {
    renderDialog({ folder: existing() });
    expect(screen.queryByLabelText(/jetzt mitdrucken/)).not.toBeInTheDocument();
  });
});

describe("FolderDialog fit-to-page", () => {
  it("defaults to on when creating a folder", () => {
    renderDialog();
    expect(screen.getByLabelText("Inhalt an Seite anpassen")).toBeChecked();
  });

  it("prefills fit-to-page from the folder when editing", () => {
    renderDialog({ folder: existing({ fit_to_page: 0 }) });
    expect(screen.getByLabelText("Inhalt an Seite anpassen")).not.toBeChecked();
  });

  it("submits fit_to_page true by default and false once switched off", async () => {
    const { onSubmit } = renderDialog();
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    fireEvent.change(screen.getByLabelText("Ordner"), {
      target: { value: "/Users/tim/Scans" },
    });
    fireEvent.click(screen.getByLabelText("Inhalt an Seite anpassen"));
    fireEvent.click(screen.getByRole("button", { name: "Anlegen" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].folder.fit_to_page).toBe(false);
  });

  it("explains that oversized content is shrunk either way", () => {
    renderDialog();
    expect(screen.getByTestId("fit-to-page-hint")).toHaveTextContent(
      "Zu große Inhalte werden in jedem Fall verkleinert.",
    );
  });
});

describe("FolderDialog form", () => {
  it("spells out the two-interval stability delay", () => {
    renderDialog();
    expect(screen.getByTestId("interval-hint")).toHaveTextContent(
      "bis zu 10 Sekunden nach dem Auftauchen gedruckt",
    );
  });

  it("prefills the interval from the default poll setting when creating", () => {
    renderDialog({ defaultPollIntervalSecs: 1 });
    expect(screen.getByLabelText("Prüfintervall (Sekunden)")).toHaveValue(1);
  });

  it("ignores the default poll setting when editing an existing folder", () => {
    renderDialog({ folder: existing({ poll_interval_secs: 30 }), defaultPollIntervalSecs: 1 });
    expect(screen.getByLabelText("Prüfintervall (Sekunden)")).toHaveValue(30);
  });

  it("uses the native folder picker", async () => {
    openMock.mockResolvedValue("/Users/tim/Belege");
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Durchsuchen …" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Ordner")).toHaveValue("/Users/tim/Belege"),
    );
    expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false });
  });

  it("refuses to submit without a name, a path and at least one file type", () => {
    const { onSubmit } = renderDialog();
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Scanner" } });
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Ordner"), { target: { value: "/tmp/x" } });
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "PDF" }));
    expect(screen.getByRole("button", { name: "Anlegen" })).toBeDisabled();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("prefills every field when editing", () => {
    renderDialog({ folder: existing() });
    expect(screen.getByLabelText("Name")).toHaveValue("Scanner");
    expect(screen.getByLabelText("Ordner")).toHaveValue("/Users/tim/Scans");
    expect(screen.getByLabelText("Prüfintervall (Sekunden)")).toHaveValue(30);
    expect(screen.getByLabelText("Kopien")).toHaveValue(3);
    expect(screen.getByLabelText("Duplex")).toHaveTextContent("Duplex (lange Kante)");
    expect(screen.getByLabelText("Farbe")).toHaveTextContent("Farbe");
    expect(screen.getByLabelText("Nach dem Druck")).toHaveTextContent("Liegen lassen");
    expect(screen.getByLabelText("Inhalt an Seite anpassen")).toBeChecked();
    expect(screen.getByRole("button", { name: "Speichern" })).toBeInTheDocument();
  });
});
