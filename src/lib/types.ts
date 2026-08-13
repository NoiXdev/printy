export type DuplexMode = "simplex" | "long_edge" | "short_edge";
export type ColorMode = "color" | "mono";
export type PostAction = "move" | "keep" | "delete";
export type FolderStatus = "ok" | "path_missing" | "printer_missing";
export type JobState = "queued" | "printing" | "retrying" | "done" | "failed";
export type NotificationMode = "all" | "errors" | "off";

/**
 * The keys `get_settings_cmd` always returns, defaulted server-side. There is
 * deliberately no `theme` key: the theme is a per-device display preference and
 * lives in `localStorage`, because it must be applied before the first paint.
 */
export type SettingKey =
  | "notification_mode"
  | "autostart"
  | "start_minimized"
  | "sumatra_path"
  | "pdfium_path"
  | "default_poll_interval_secs"
  | "user_paused";

/** A row of `watch_folder`. `enabled` and `fit_to_page` are SQLite integer booleans. */
export interface WatchFolder {
  id: number;
  name: string;
  path: string;
  enabled: number;
  poll_interval_secs: number;
  /** JSON array of lowercase extensions, e.g. `["pdf","png"]`. */
  file_types: string;
  printer_name: string;
  copies: number;
  duplex: DuplexMode;
  color_mode: ColorMode;
  post_action: PostAction;
  status: FolderStatus;
  created_at: string;
  updated_at: string;
  /** Per-folder "scale to fit page" mode. Defaults to true on creation. */
  fit_to_page: number;
}

/** The payload `create_folder_cmd` and `update_folder_cmd` deserialize. */
export interface NewFolder {
  name: string;
  path: string;
  poll_interval_secs: number;
  file_types: string[];
  printer_name: string;
  copies: number;
  duplex: DuplexMode;
  color_mode: ColorMode;
  post_action: PostAction;
  fit_to_page: boolean;
}

/** A row of `print_job` — queue, history and dedup ledger in one. */
export interface PrintJob {
  id: number;
  folder_id: number;
  file_path: string;
  file_name: string;
  size_bytes: number;
  mtime_ms: number;
  sha256: string;
  state: JobState;
  attempts: number;
  printer_name: string;
  copies: number;
  duplex: DuplexMode;
  color_mode: ColorMode;
  error_kind: string | null;
  error_message: string | null;
  next_attempt_at: number | null;
  enqueued_at: string;
  started_at: string | null;
  finished_at: string | null;
  /** Snapshotted from the folder's `fit_to_page` at enqueue time. */
  fit_to_page: number;
}

export interface PrinterInfo {
  name: string;
  is_default: boolean;
}

export interface PrinterCapabilities {
  duplex: boolean;
  color: boolean;
  copies: boolean;
}

export interface AppStatus {
  active_folders: number;
  printed_today: number;
  waiting: number;
  failed: number;
  user_paused: boolean;
}

export type AppSettings = Record<SettingKey, string>;

/** Payload of `printy://job`. The debug name of `queue::worker::QueueOutcome`. */
export type JobOutcome = "Printed" | "Retried" | "Failed";
export interface JobEvent {
  outcome: JobOutcome;
}

/** Payload of `printy://folder`. */
export interface FolderEvent {
  folder_id: number;
  enqueued: number;
  path_missing: boolean;
}

/** Payload of `printy://queue`. `reason` is only present while held. */
export interface QueueEvent {
  held: boolean;
  reason?: string;
}

/**
 * Payload of `printy://paused`. Deliberately separate from `QueueEvent`: the
 * user's pause switch and the printer hold are independent states, and
 * listeners that key off `held` must never see this event.
 */
export interface PausedEvent {
  paused: boolean;
}
