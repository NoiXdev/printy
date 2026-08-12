CREATE TABLE watch_folder (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  name               TEXT    NOT NULL,
  path               TEXT    NOT NULL UNIQUE,
  enabled            INTEGER NOT NULL DEFAULT 1,
  poll_interval_secs INTEGER NOT NULL DEFAULT 5,
  file_types         TEXT    NOT NULL DEFAULT '["pdf"]',
  printer_name       TEXT    NOT NULL,
  copies             INTEGER NOT NULL DEFAULT 1,
  duplex             TEXT    NOT NULL DEFAULT 'simplex',
  color_mode         TEXT    NOT NULL DEFAULT 'mono',
  post_action        TEXT    NOT NULL DEFAULT 'move',
  status             TEXT    NOT NULL DEFAULT 'ok',
  created_at         TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at         TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE print_job (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  folder_id       INTEGER NOT NULL REFERENCES watch_folder(id) ON DELETE CASCADE,
  file_path       TEXT    NOT NULL,
  file_name       TEXT    NOT NULL,
  size_bytes      INTEGER NOT NULL,
  mtime_ms        INTEGER NOT NULL,
  sha256          TEXT    NOT NULL,
  state           TEXT    NOT NULL,
  attempts        INTEGER NOT NULL DEFAULT 0,
  printer_name    TEXT    NOT NULL,
  copies          INTEGER NOT NULL DEFAULT 1,
  duplex          TEXT    NOT NULL DEFAULT 'simplex',
  color_mode      TEXT    NOT NULL DEFAULT 'mono',
  error_kind      TEXT,
  error_message   TEXT,
  next_attempt_at INTEGER,
  enqueued_at     TEXT    NOT NULL DEFAULT (datetime('now')),
  started_at      TEXT,
  finished_at     TEXT
);

CREATE INDEX idx_job_state       ON print_job(state);
CREATE INDEX idx_job_folder_hash ON print_job(folder_id, sha256);
CREATE INDEX idx_job_folder_stamp
  ON print_job(folder_id, file_name, size_bytes, mtime_ms);

CREATE TABLE app_setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
