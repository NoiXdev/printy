import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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
    case "set_global_paused_cmd":
    case "set_folder_enabled_cmd":
    case "scan_now_cmd":
      return undefined;
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
      <Folders />
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

  it("opens the dialog from the add tile", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Ordner hinzufügen" }));
    expect(await screen.findByRole("dialog", { name: "Ordner hinzufügen" })).toBeInTheDocument();
  });

  it("triggers a manual scan for a single folder", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Jetzt scannen" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("scan_now_cmd", { id: 1 }),
    );
  });
});
