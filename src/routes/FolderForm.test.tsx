import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import FolderForm from "./FolderForm";
import type { WatchFolder } from "../lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const FOLDERS: WatchFolder[] = [
  {
    id: 7,
    name: "Rechnungen",
    path: "/Users/tim/Rechnungen",
    enabled: 1,
    poll_interval_secs: 12,
    file_types: '["pdf"]',
    printer_name: "Brother MFC",
    copies: 2,
    duplex: "long_edge",
    color_mode: "color",
    post_action: "keep",
    status: "ok",
    created_at: "2026-08-01 09:00:00",
    updated_at: "2026-08-01 09:00:00",
    fit_to_page: 1,
  },
];

function respond(cmd: string): unknown {
  switch (cmd) {
    case "list_folders_cmd":
      return FOLDERS;
    case "list_printers_cmd":
      return [
        { name: "Brother MFC", is_default: false },
        { name: "HP LaserJet", is_default: true },
      ];
    case "printer_capabilities_cmd":
      return { duplex: true, color: true, copies: true };
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
    case "count_existing_files_cmd":
      return 0;
    case "create_folder_cmd":
    case "update_folder_cmd":
      return FOLDERS[0];
    default:
      throw new Error(`unexpected command ${cmd}`);
  }
}

/** A stand-in for the Ordner list -- FolderForm only needs to prove it lands
 * back there, not what that screen renders. */
function renderAt(path: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/" element={<div data-testid="folders-list">Ordner-Liste</div>} />
          <Route path="/ordner/neu" element={<FolderForm />} />
          <Route path="/ordner/:id" element={<FolderForm />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("FolderForm", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => respond(cmd));
  });

  it("creates a folder against the system default printer", async () => {
    renderAt("/ordner/neu");
    expect(await screen.findByRole("heading", { name: "Ordner hinzufügen" })).toBeInTheDocument();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("printer_capabilities_cmd", {
        printer: "HP LaserJet",
      }),
    );

    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Neuer Ordner" } });
    fireEvent.change(screen.getByLabelText("Ordner"), { target: { value: "/tmp/neu" } });
    fireEvent.click(screen.getByRole("button", { name: "Anlegen" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "create_folder_cmd",
        expect.objectContaining({
          folder: expect.objectContaining({ name: "Neuer Ordner", printer_name: "HP LaserJet" }),
          printExisting: false,
        }),
      ),
    );
    expect(await screen.findByTestId("folders-list")).toBeInTheDocument();
  });

  it("loads the folder named by the route id and probes its own printer's capabilities", async () => {
    renderAt("/ordner/7");
    expect(await screen.findByLabelText("Name")).toHaveValue("Rechnungen");
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("printer_capabilities_cmd", {
        printer: "Brother MFC",
      }),
    );
  });

  it("saves an edit through update_folder_cmd with that id and returns to the list", async () => {
    renderAt("/ordner/7");
    await screen.findByLabelText("Name");
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Rechnungen (neu)" } });
    fireEvent.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "update_folder_cmd",
        expect.objectContaining({
          id: 7,
          folder: expect.objectContaining({ name: "Rechnungen (neu)" }),
        }),
      ),
    );
    expect(await screen.findByTestId("folders-list")).toBeInTheDocument();
  });

  it("sends the user back to the list for an id that no longer exists", async () => {
    renderAt("/ordner/999");
    expect(
      await screen.findByRole("heading", { name: "Ordner nicht gefunden" }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Zurück zur Ordnerliste" }));
    expect(await screen.findByTestId("folders-list")).toBeInTheDocument();
  });
});
