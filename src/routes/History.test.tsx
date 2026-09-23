import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom";
import { invoke } from "@tauri-apps/api/core";
import History from "./History";
import type { PrintJob, WatchFolder } from "../lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function job(overrides: Partial<PrintJob>): PrintJob {
  return {
    id: 1,
    folder_id: 1,
    file_path: "/Users/tim/Scans/a.pdf",
    file_name: "a.pdf",
    size_bytes: 10,
    mtime_ms: 1,
    sha256: "h",
    state: "done",
    attempts: 0,
    printer_name: "HP LaserJet",
    copies: 1,
    duplex: "simplex",
    color_mode: "mono",
    error_kind: null,
    error_message: null,
    next_attempt_at: null,
    enqueued_at: "2026-08-12 09:00:00",
    started_at: "2026-08-12 09:00:01",
    finished_at: "2026-08-12 09:00:05",
    fit_to_page: 1,
    ...overrides,
  };
}

const JOBS: PrintJob[] = [
  job({ id: 1, file_name: "rechnung.pdf", state: "done" }),
  job({
    id: 2,
    file_name: "kaputt.pdf",
    state: "failed",
    error_kind: "file",
    error_message: "PDF nicht lesbar",
  }),
];

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

function renderScreen() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <History />
    </QueryClientProvider>,
  );
}

describe("History", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "list_jobs_cmd") {
        return args?.onlyFailed === true ? JOBS.filter((j) => j.state === "failed") : JOBS;
      }
      if (cmd === "list_folders_cmd") return FOLDERS;
      if (cmd === "reprint_job_cmd") return undefined;
      throw new Error(`unexpected command ${cmd}`);
    });
  });

  it("lists file, folder, printer, time and outcome", async () => {
    renderScreen();
    expect(await screen.findByText("rechnung.pdf")).toBeInTheDocument();
    const row = screen.getByTestId("job-row-1");
    expect(row).toHaveTextContent("Scanner");
    expect(row).toHaveTextContent("HP LaserJet");
    expect(row).toHaveTextContent("12.08.2026");
    expect(row).toHaveTextContent("Gedruckt");
  });

  it("filters to failures only and back", async () => {
    renderScreen();
    expect(await screen.findByText("rechnung.pdf")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Nur Fehler"));
    await waitFor(() => expect(screen.queryByText("rechnung.pdf")).not.toBeInTheDocument());
    expect(screen.getByText("kaputt.pdf")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("list_jobs_cmd", {
      onlyFailed: true,
      limit: 300,
    });

    fireEvent.click(screen.getByLabelText("Nur Fehler"));
    await waitFor(() => expect(screen.getByText("rechnung.pdf")).toBeInTheDocument());
  });

  it("shows the error message on a failed job", async () => {
    renderScreen();
    expect(await screen.findByText("PDF nicht lesbar")).toBeInTheDocument();
  });

  it("names the folder a job came from even after that folder is gone", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "list_jobs_cmd") return [job({ id: 7, folder_id: 99 })];
      if (cmd === "list_folders_cmd") return FOLDERS;
      throw new Error(`unexpected command ${cmd}`);
    });
    renderScreen();
    expect(await screen.findByText("Gelöschter Ordner", { exact: false })).toBeInTheDocument();
  });

  it("reprints a single row", async () => {
    renderScreen();
    await screen.findByText("rechnung.pdf");
    fireEvent.click(screen.getAllByRole("button", { name: "Erneut drucken" })[0]);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("reprint_job_cmd", { id: 1 }));
  });

  it("offers no reprint on a job that has not finished yet", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "list_jobs_cmd") {
        return [job({ id: 3, file_name: "laeuft.pdf", state: "printing" })];
      }
      if (cmd === "list_folders_cmd") return FOLDERS;
      throw new Error(`unexpected command ${cmd}`);
    });
    renderScreen();
    await screen.findByText("laeuft.pdf");
    expect(screen.queryByRole("button", { name: "Erneut drucken" })).not.toBeInTheDocument();
  });

  it("reports why a reprint was refused instead of failing silently", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "list_jobs_cmd") return JOBS;
      if (cmd === "list_folders_cmd") return FOLDERS;
      if (cmd === "reprint_job_cmd") throw "Datei existiert nicht mehr";
      throw new Error(`unexpected command ${cmd}`);
    });
    renderScreen();
    await screen.findByText("rechnung.pdf");
    fireEvent.click(screen.getAllByRole("button", { name: "Erneut drucken" })[0]);
    expect(await screen.findByText("Datei existiert nicht mehr")).toBeInTheDocument();
  });

  it("shows an empty state when nothing has been printed yet", async () => {
    invokeMock.mockImplementation(async (cmd: string) =>
      cmd === "list_jobs_cmd" ? [] : FOLDERS,
    );
    renderScreen();
    expect(await screen.findByText("Noch nichts gedruckt")).toBeInTheDocument();
  });

  it("says no failures rather than nothing printed when the filter is on", async () => {
    invokeMock.mockImplementation(async (cmd: string) =>
      cmd === "list_jobs_cmd" ? [] : FOLDERS,
    );
    renderScreen();
    await screen.findByText("Noch nichts gedruckt");
    fireEvent.click(screen.getByLabelText("Nur Fehler"));
    expect(await screen.findByText("Keine Fehler")).toBeInTheDocument();
  });
});
