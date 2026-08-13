import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  AppStatus,
  NewFolder,
  PrintJob,
  PrinterCapabilities,
  PrinterInfo,
  ScanAllResult,
  SettingKey,
  WatchFolder,
} from "./types";

/**
 * The only place in the frontend that talks to Tauri. Command names and
 * parameter names come from the core engine plan's Task 17; camelCase argument
 * keys are converted to snake_case parameters by Tauri itself.
 */
export const api = {
  listPrinters: () => invoke<PrinterInfo[]>("list_printers_cmd"),
  printerCapabilities: (printer: string) =>
    invoke<PrinterCapabilities>("printer_capabilities_cmd", { printer }),

  listFolders: () => invoke<WatchFolder[]>("list_folders_cmd"),
  createFolder: (folder: NewFolder, printExisting: boolean) =>
    invoke<WatchFolder>("create_folder_cmd", { folder, printExisting }),
  updateFolder: (id: number, folder: NewFolder) =>
    invoke<WatchFolder>("update_folder_cmd", { id, folder }),
  deleteFolder: (id: number) => invoke<void>("delete_folder_cmd", { id }),
  setFolderEnabled: (id: number, enabled: boolean) =>
    invoke<void>("set_folder_enabled_cmd", { id, enabled }),
  scanNow: (id: number) => invoke<number>("scan_now_cmd", { id }),
  scanAllFolders: () => invoke<ScanAllResult>("scan_all_folders_cmd"),

  listJobs: (onlyFailed: boolean, limit: number) =>
    invoke<PrintJob[]>("list_jobs_cmd", { onlyFailed, limit }),
  reprintJob: (id: number) => invoke<void>("reprint_job_cmd", { id }),

  getSettings: () => invoke<AppSettings>("get_settings_cmd"),
  updateSetting: (key: SettingKey, value: string) =>
    invoke<void>("update_setting_cmd", { key, value }),
  setGlobalPaused: (paused: boolean) => invoke<void>("set_global_paused_cmd", { paused }),
  getStatus: () => invoke<AppStatus>("get_status_cmd"),

  countExistingFiles: (path: string, fileTypes: string[]) =>
    invoke<number>("count_existing_files_cmd", { path, fileTypes }),
  getAutostart: () => invoke<boolean>("get_autostart_cmd"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart_cmd", { enabled }),
};
