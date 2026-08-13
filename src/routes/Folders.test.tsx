import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import Folders from "./Folders";
import type { PrintJob, WatchFolder } from "../lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const FOLDERS: WatchFolder[] = [
  {
    id: 1,
    name: "Scanner",
    path: "/Users/tim/Scans",
    enabled: 1,
    poll_interval_secs: 5,
    file_types: '["pdf"]',
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    post_action: "move",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    fit_to_page: 1,
  },
];

const JOBS: PrintJob[] = [];

function respond(cmd: string, args?: Record<string, unknown>): unknown {
  switch (cmd) {
    case "list_folders_cmd":
      return FOLDERS;
    case "list_jobs_cmd":
      return JOBS;
    case "list_printers_cmd":
      return [{ name: "HP LaserJet", is_default: true }];
    case "printer_capabilities_cmd":
      return { duplex: true, color: true, copies: true };
    case "get_status_cmd":
      return {
        active_folders: 3,
        printed_today: 12,
        waiting: 0,
        failed: 0,
        user_paused: false,
      };
    case "get_settings_cmd":
      return {
        notification_mode: "all",
        autostart: "0",
        start_minimized: "0",
        sumatra_path: "",
        pdfium_path: "",
        default_poll_interval_secs: "5",
        user_paused: "0",
      };
    case "set_global_paused_cmd":
    case "set_folder_enabled_cmd":
    case "scan_now_cmd":
    case "delete_folder_cmd":
      return undefined;
    case "scan_all_folders_cmd":
      return { folders_scanned: 3, enqueued: 7 };
    case "folder_delete_impact_cmd":
      return { waiting: 3, history: 47 };
    default:
      throw new Error(`unexpected command ${cmd} ${JSON.stringify(args)}`);
  }
}

function renderScreen() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/"]}>
        <Routes>
          <Route path="/" element={<Folders />} />
          {/* Stand-ins for the real FolderForm route: Folders only needs to
              prove it navigates there, not what that page renders. */}
          <Route path="/ordner/neu" element={<div>Neuer-Ordner-Platzhalter</div>} />
          <Route
            path="/ordner/:id"
            element={<div data-testid="edit-route-placeholder">Ordner-bearbeiten-Platzhalter</div>}
          />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Folders", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) =>
      respond(cmd, args),
    );
  });

  it("renders the aggregate header from get_status_cmd", async () => {
    renderScreen();
    await waitFor(() =>
      expect(screen.getByTestId("aggregate")).toHaveTextContent(
        "3 aktiv · 12 heute gedruckt",
      ),
    );
  });

  it("renders one card per folder plus the add tile", async () => {
    renderScreen();
    await waitFor(() => expect(screen.getByText("Scanner")).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Ordner hinzufügen" })).toBeInTheDocument();
  });

  it("sends the global pause switch straight to set_global_paused_cmd", async () => {
    renderScreen();
    const toggle = await screen.findByLabelText("Alles pausieren");
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_global_paused_cmd", { paused: true }),
    );
  });

  it("navigates to the create-folder route from the add tile", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Ordner hinzufügen" }));
    expect(await screen.findByText("Neuer-Ordner-Platzhalter")).toBeInTheDocument();
  });

  it("navigates to the edit route for a folder's own id from its card menu", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Aktionen für Scanner" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Bearbeiten" }));
    expect(await screen.findByTestId("edit-route-placeholder")).toBeInTheDocument();
  });

  it("triggers a manual scan for a single folder from its card menu", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Aktionen für Scanner" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Jetzt scannen" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("scan_now_cmd", { id: 1 }),
    );
  });

  it("rescans every folder at once, disables the button while running, and reports the total", async () => {
    // A deferred promise makes the in-flight window observable instead of
    // racing against a mock that resolves in the same microtask.
    let resolveScan!: (value: unknown) => void;
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "scan_all_folders_cmd") {
        return new Promise((resolve) => {
          resolveScan = resolve;
        });
      }
      return respond(cmd, args);
    });

    renderScreen();
    const button = await screen.findByRole("button", { name: "Alle Ordner neu einlesen" });
    fireEvent.click(button);

    await waitFor(() => expect(button).toBeDisabled());
    expect(invokeMock).toHaveBeenCalledWith("scan_all_folders_cmd");

    resolveScan({ folders_scanned: 3, enqueued: 7 });

    await waitFor(() =>
      expect(screen.getByTestId("rescan-message")).toHaveTextContent(
        "7 Dateien aus 3 Ordnern eingelesen.",
      ),
    );
    expect(button).not.toBeDisabled();
  });

  it("asks for the impact counts and shows them concretely before deleting", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Aktionen für Scanner" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Löschen" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("folder_delete_impact_cmd", { id: 1 }),
    );
    expect(
      await screen.findByText(
        "3 wartende Aufträge und 47 Einträge im Verlauf werden mitgelöscht. " +
          "Die Dateien im überwachten Ordner selbst werden dabei nicht angetastet.",
      ),
    ).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("delete_folder_cmd", expect.anything());
  });

  it("cancelling the delete dialog never calls delete_folder_cmd", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Aktionen für Scanner" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Löschen" }));
    await screen.findByRole("alertdialog");

    fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("delete_folder_cmd", expect.anything());
  });

  it("confirming the delete dialog calls delete_folder_cmd for the right folder", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Aktionen für Scanner" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Löschen" }));
    await screen.findByRole("alertdialog");

    fireEvent.click(screen.getByRole("button", { name: "Endgültig löschen" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("delete_folder_cmd", { id: 1 }),
    );
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });
});
