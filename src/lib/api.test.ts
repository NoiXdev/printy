import { describe, it, expect, beforeEach, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";
import type { NewFolder } from "./types";

// Tauri is not reachable in jsdom. tabsy never had to mock it because nothing
// under test called a command; every Printy screen does, so the whole core
// module is replaced here and in every component test.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const NEW_FOLDER: NewFolder = {
  name: "Scans",
  path: "/Users/tim/Scans",
  poll_interval_secs: 5,
  file_types: ["pdf"],
  printer_name: "Brother MFC",
  copies: 1,
  duplex: "simplex",
  color_mode: "mono",
  post_action: "move",
  fit_to_page: true,
};

describe("api", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("calls the folder commands with the parameter names the backend declares", async () => {
    await api.listFolders();
    expect(invokeMock).toHaveBeenCalledWith("list_folders_cmd");

    await api.createFolder(NEW_FOLDER, false);
    expect(invokeMock).toHaveBeenCalledWith("create_folder_cmd", {
      folder: NEW_FOLDER,
      printExisting: false,
    });

    await api.updateFolder(7, NEW_FOLDER);
    expect(invokeMock).toHaveBeenCalledWith("update_folder_cmd", {
      id: 7,
      folder: NEW_FOLDER,
    });

    await api.setFolderEnabled(7, false);
    expect(invokeMock).toHaveBeenCalledWith("set_folder_enabled_cmd", {
      id: 7,
      enabled: false,
    });

    await api.scanNow(7);
    expect(invokeMock).toHaveBeenCalledWith("scan_now_cmd", { id: 7 });

    await api.deleteFolder(7);
    expect(invokeMock).toHaveBeenCalledWith("delete_folder_cmd", { id: 7 });
  });

  it("calls the job commands", async () => {
    await api.listJobs(true, 200);
    expect(invokeMock).toHaveBeenCalledWith("list_jobs_cmd", {
      onlyFailed: true,
      limit: 200,
    });

    await api.reprintJob(3);
    expect(invokeMock).toHaveBeenCalledWith("reprint_job_cmd", { id: 3 });
  });

  it("calls the printer commands", async () => {
    await api.listPrinters();
    expect(invokeMock).toHaveBeenCalledWith("list_printers_cmd");

    await api.printerCapabilities("Brother MFC");
    expect(invokeMock).toHaveBeenCalledWith("printer_capabilities_cmd", {
      printer: "Brother MFC",
    });
  });

  it("calls the settings, status and shell commands", async () => {
    await api.getSettings();
    expect(invokeMock).toHaveBeenCalledWith("get_settings_cmd");

    await api.updateSetting("notification_mode", "errors");
    expect(invokeMock).toHaveBeenCalledWith("update_setting_cmd", {
      key: "notification_mode",
      value: "errors",
    });

    await api.setGlobalPaused(true);
    expect(invokeMock).toHaveBeenCalledWith("set_global_paused_cmd", { paused: true });

    await api.getStatus();
    expect(invokeMock).toHaveBeenCalledWith("get_status_cmd");

    await api.countExistingFiles("/Users/tim/Scans", ["pdf"]);
    expect(invokeMock).toHaveBeenCalledWith("count_existing_files_cmd", {
      path: "/Users/tim/Scans",
      fileTypes: ["pdf"],
    });

    await api.getAutostart();
    expect(invokeMock).toHaveBeenCalledWith("get_autostart_cmd");

    await api.setAutostart(true);
    expect(invokeMock).toHaveBeenCalledWith("set_autostart_cmd", { enabled: true });
  });
});
