# Printy Core Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the headless engine of Printy — folder watching, file intake, a serial print queue with retry semantics, and pluggable print backends for Windows, macOS and tests — exposed to the frontend through Tauri commands.

**Architecture:** A Tauri 2 desktop app whose Rust backend runs one Tokio task per watched folder plus exactly one serial queue worker. SQLite (sqlx, runtime-checked queries) is the single source of truth for configuration, queue and history. Printing sits behind a `PrintBackend` trait with four implementations selected by `cfg`, so every layer above it is testable on macOS without a printer.

**Tech Stack:** Rust 2021, Tauri 2, sqlx 0.8 + SQLite, tokio, thiserror, pdfium-render 0.8, image 0.25, sha2, chrono, `windows` crate (Windows only). Frontend scaffold only in this plan: React 19 + TypeScript + Vite.

This plan covers spec sections 3, 5, 6, 7, 8, 9, 11, 12 and 13. The user interface (spec section 10), tray, autostart, notifications and branding (spec sections 4 and 10) are covered by a second plan.

## Global Constraints

- Reference spec: `docs/superpowers/specs/2026-08-12-printy-design.md`. It is authoritative; where this plan and the spec disagree, stop and ask.
- Conventions are inherited from the sibling project `/Users/noidee/_dev/tabs-manager`. Match it rather than inventing new patterns.
- Code, identifiers, comments and commit messages are **English**. User-facing strings (error messages surfaced in the UI) are **German**.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `test:`, `chore:`, `refactor:`, `build:`).
- sqlx is used **runtime-checked only** — `sqlx::query_as::<_, T>(sql)`, never the `query!`/`query_as!` macros. No `DATABASE_URL`, no `.sqlx/` offline data.
- Migrations live in `src-tauri/migrations/`, named `NNNN_snake_case.sql`, zero-padded to 4, strictly additive. No down-migrations.
- The db layer returns `Result<T, sqlx::Error>`. Only Tauri commands return `AppResult<T>`.
- Tauri commands are suffixed `_cmd`, take `state: State<'_, AppState>` as the first parameter, and are one-line wrappers over testable functions.
- Tests are in-crate `#[cfg(test)] mod tests` blocks opening with `use super::*;`. Async tests use `#[tokio::test]`. SQLite tests use `connect("sqlite::memory:")`. **No dev-dependencies are added** — no `tempfile`, no `rstest`. Filesystem tests use `std::env::temp_dir()` with a uniquifier and best-effort cleanup, as in `tabs-manager/src-tauri/src/autofetch/local.rs:75`.
- `Pdfium` is not `Send`. Every pdfium call happens inside a synchronous function, bound per call via the candidate-directory search copied from `tabs-manager/src-tauri/src/autofetch/pdf.rs:6`. Printing is invoked from async code through `tokio::task::spawn_blocking`.
- Background tasks use `tauri::async_runtime::spawn` with an owned `AppHandle`, and clone values out of `State` inside a scoped block **before** any `.await` — the `State` guard is not `Send`.
- Poll interval floor is 1 second; default 5 seconds. Retry backoff is exactly 5 s, 30 s, 120 s with `MAX_ATTEMPTS = 3`.
- Reserved subdirectory names `printed` and `failed` are never scanned.
- Every task ends with a passing `cargo test` and a commit.

---

### Task 1: Project scaffold, error type, database with migrations

**Files:**
- Create: whole project skeleton in the repository root
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/db/mod.rs`, `src-tauri/src/db/models.rs`
- Create: `src-tauri/migrations/0001_init.sql`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `error::AppError`, `error::AppResult<T>`; `db::Db` (= `SqlitePool`), `db::connect(url: &str) -> Result<Db, sqlx::Error>`; row structs `db::models::WatchFolder`, `db::models::PrintJob`.

- [ ] **Step 1: Scaffold the project**

Run in the repository root (`/Users/noidee/_dev/folder_printe`):

```bash
npm create tauri-app@latest . -- --template react-ts --manager npm --identifier com.noidee.printy --app-name printy
npm install
```

Then replace `src-tauri/Cargo.toml`'s `[lib]` and `[dependencies]` sections with:

```toml
[lib]
name = "printy_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-opener = "2"
tauri-plugin-notification = "2"
tauri-plugin-autostart = "2"
tauri-plugin-dialog = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }
tokio = { version = "1", features = ["full"] }
thiserror = "1"
chrono = { version = "0.4", features = ["serde"] }
pdfium-render = "0.8"
image = "0.25"
sha2 = "0.10"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_Graphics_Printing",
  "Win32_Storage_Xps",
  "Win32_System_Registry",
] }
```

- [ ] **Step 2: Write the failing test**

Create `src-tauri/src/db/mod.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_run_on_memory_db() {
        let db = connect("sqlite::memory:").await.expect("connect");
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_folder")
            .fetch_one(&db)
            .await
            .expect("query");
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn job_and_setting_tables_exist() {
        let db = connect("sqlite::memory:").await.expect("connect");
        let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM print_job")
            .fetch_one(&db).await.expect("print_job");
        let settings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_setting")
            .fetch_one(&db).await.expect("app_setting");
        assert_eq!((jobs, settings), (0, 0));
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd src-tauri && cargo test db::tests -- --nocapture`
Expected: FAIL — `cannot find function 'connect' in this scope`.

- [ ] **Step 4: Write the error type**

Create `src-tauri/src/error.rs`:

```rust
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Datenbankfehler: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Dateisystemfehler: {0}")]
    Io(#[from] std::io::Error),
    #[error("Druckfehler: {0}")]
    Print(String),
    #[error("{0}")]
    Other(String),
}

// Tauri commands must return Result<T, E: Serialize>; flatten to a plain string
// so no error taxonomy leaks into TypeScript.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 5: Write the migration**

Create `src-tauri/migrations/0001_init.sql`:

```sql
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
```

- [ ] **Step 6: Write the db module and row structs**

Prepend to `src-tauri/src/db/mod.rs` (above the test module):

```rust
pub mod models;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

pub type Db = SqlitePool;

/// Opens the pool and runs all pending migrations.
pub async fn connect(url: &str) -> Result<Db, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

Create `src-tauri/src/db/models.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchFolder {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub enabled: i64,
    pub poll_interval_secs: i64,
    pub file_types: String,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
    pub post_action: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl WatchFolder {
    /// `file_types` is stored as a JSON array of lowercase extensions.
    pub fn types(&self) -> Vec<String> {
        serde_json::from_str(&self.file_types).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PrintJob {
    pub id: i64,
    pub folder_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub mtime_ms: i64,
    pub sha256: String,
    pub state: String,
    pub attempts: i64,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub next_attempt_at: Option<i64>,
    pub enqueued_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
```

Replace the module list at the top of `src-tauri/src/lib.rs` with:

```rust
mod db;
mod error;
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd src-tauri && cargo test`
Expected: PASS — 2 tests.

- [ ] **Step 8: Commit**

```bash
git init
git add -A
git commit -m "feat: scaffold printy with sqlite schema and error type"
```

---

### Task 2: Watch folder CRUD

**Files:**
- Create: `src-tauri/src/db/folders.rs`
- Modify: `src-tauri/src/db/mod.rs` (add `pub mod folders;`)

**Interfaces:**
- Consumes: `db::Db`, `db::models::WatchFolder`.
- Produces: `db::folders::{NewFolder, create_folder, list_folders, get_folder, update_folder, delete_folder, set_folder_enabled, set_folder_status}`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/db/folders.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;

    fn sample() -> NewFolder {
        NewFolder {
            name: "Scans".into(),
            path: "/tmp/printy-scans".into(),
            poll_interval_secs: 5,
            file_types: vec!["pdf".into()],
            printer_name: "Brother".into(),
            copies: 1,
            duplex: "simplex".into(),
            color_mode: "mono".into(),
            post_action: "move".into(),
        }
    }

    #[tokio::test]
    async fn create_and_list_round_trip() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &sample()).await.unwrap();
        assert_eq!(f.name, "Scans");
        assert_eq!(f.enabled, 1);
        assert_eq!(f.types(), vec!["pdf".to_string()]);
        assert_eq!(list_folders(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn duplicate_path_is_rejected() {
        let db = connect("sqlite::memory:").await.unwrap();
        create_folder(&db, &sample()).await.unwrap();
        assert!(create_folder(&db, &sample()).await.is_err());
    }

    #[tokio::test]
    async fn interval_below_floor_is_clamped() {
        let db = connect("sqlite::memory:").await.unwrap();
        let mut n = sample();
        n.poll_interval_secs = 0;
        let f = create_folder(&db, &n).await.unwrap();
        assert_eq!(f.poll_interval_secs, MIN_POLL_INTERVAL_SECS);
    }

    #[tokio::test]
    async fn update_enabled_and_status() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &sample()).await.unwrap();
        set_folder_enabled(&db, f.id, false).await.unwrap();
        set_folder_status(&db, f.id, "path_missing").await.unwrap();
        let got = get_folder(&db, f.id).await.unwrap().unwrap();
        assert_eq!(got.enabled, 0);
        assert_eq!(got.status, "path_missing");
    }

    #[tokio::test]
    async fn delete_removes_folder() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &sample()).await.unwrap();
        delete_folder(&db, f.id).await.unwrap();
        assert!(get_folder(&db, f.id).await.unwrap().is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test db::folders`
Expected: FAIL — `cannot find type 'NewFolder' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/db/folders.rs`:

```rust
use crate::db::models::WatchFolder;
use crate::db::Db;
use serde::Deserialize;

pub const MIN_POLL_INTERVAL_SECS: i64 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct NewFolder {
    pub name: String,
    pub path: String,
    pub poll_interval_secs: i64,
    pub file_types: Vec<String>,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
    pub post_action: String,
}

fn clamp(n: &NewFolder) -> (i64, i64, String) {
    let interval = n.poll_interval_secs.max(MIN_POLL_INTERVAL_SECS);
    let copies = n.copies.max(1);
    let types = serde_json::to_string(&n.file_types).unwrap_or_else(|_| "[]".into());
    (interval, copies, types)
}

pub async fn create_folder(db: &Db, n: &NewFolder) -> Result<WatchFolder, sqlx::Error> {
    let (interval, copies, types) = clamp(n);
    sqlx::query_as::<_, WatchFolder>(
        "INSERT INTO watch_folder
           (name, path, poll_interval_secs, file_types, printer_name,
            copies, duplex, color_mode, post_action)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(&n.name).bind(&n.path).bind(interval).bind(types).bind(&n.printer_name)
    .bind(copies).bind(&n.duplex).bind(&n.color_mode).bind(&n.post_action)
    .fetch_one(db)
    .await
}

pub async fn list_folders(db: &Db) -> Result<Vec<WatchFolder>, sqlx::Error> {
    sqlx::query_as::<_, WatchFolder>("SELECT * FROM watch_folder ORDER BY id")
        .fetch_all(db)
        .await
}

pub async fn get_folder(db: &Db, id: i64) -> Result<Option<WatchFolder>, sqlx::Error> {
    sqlx::query_as::<_, WatchFolder>("SELECT * FROM watch_folder WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn update_folder(db: &Db, id: i64, n: &NewFolder) -> Result<WatchFolder, sqlx::Error> {
    let (interval, copies, types) = clamp(n);
    sqlx::query_as::<_, WatchFolder>(
        "UPDATE watch_folder SET
           name = ?, path = ?, poll_interval_secs = ?, file_types = ?,
           printer_name = ?, copies = ?, duplex = ?, color_mode = ?,
           post_action = ?, updated_at = datetime('now')
         WHERE id = ? RETURNING *",
    )
    .bind(&n.name).bind(&n.path).bind(interval).bind(types).bind(&n.printer_name)
    .bind(copies).bind(&n.duplex).bind(&n.color_mode).bind(&n.post_action).bind(id)
    .fetch_one(db)
    .await
}

pub async fn delete_folder(db: &Db, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM watch_folder WHERE id = ?")
        .bind(id).execute(db).await?;
    Ok(())
}

pub async fn set_folder_enabled(db: &Db, id: i64, enabled: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watch_folder SET enabled = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(enabled as i64).bind(id).execute(db).await?;
    Ok(())
}

pub async fn set_folder_status(db: &Db, id: i64, status: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE watch_folder SET status = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(status).bind(id).execute(db).await?;
    Ok(())
}
```

Add `pub mod folders;` to the top of `src-tauri/src/db/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test db::folders`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db
git commit -m "feat: add watch folder CRUD with interval clamping"
```

---

### Task 3: Job ledger, state transitions and crash recovery

**Files:**
- Create: `src-tauri/src/db/jobs.rs`
- Modify: `src-tauri/src/db/mod.rs` (add `pub mod jobs;`)

**Interfaces:**
- Consumes: `db::Db`, `db::models::PrintJob`, `db::folders::create_folder` (tests only).
- Produces: `db::jobs::{JobState, NewJob, enqueue_job, next_due_job, mark_printing, mark_done, mark_retrying, mark_failed, requeue_job, recover_interrupted, job_done_for_hash, job_seen_for_stamp, list_jobs, get_job}`.

Note: `Discovered` and `Stabilizing` from the spec's state machine are in-memory watcher stages. A job enters the database at `Queued`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/db/jobs.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;
    use crate::db::folders::{create_folder, NewFolder};

    async fn setup() -> (crate::db::Db, i64) {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: "/tmp/f".into(), poll_interval_secs: 5,
            file_types: vec!["pdf".into()], printer_name: "P".into(), copies: 1,
            duplex: "simplex".into(), color_mode: "mono".into(), post_action: "move".into(),
        }).await.unwrap();
        (db, f.id)
    }

    fn job(folder_id: i64, name: &str, hash: &str) -> NewJob {
        NewJob {
            folder_id,
            file_path: format!("/tmp/f/{name}"),
            file_name: name.into(),
            size_bytes: 100,
            mtime_ms: 1_700_000_000_000,
            sha256: hash.into(),
            printer_name: "P".into(),
            copies: 1,
            duplex: "simplex".into(),
            color_mode: "mono".into(),
        }
    }

    #[tokio::test]
    async fn enqueue_then_pick_up_in_fifo_order() {
        let (db, fid) = setup().await;
        enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        enqueue_job(&db, &job(fid, "b.pdf", "h2")).await.unwrap();
        let first = next_due_job(&db, 0).await.unwrap().unwrap();
        assert_eq!(first.file_name, "a.pdf");
        assert_eq!(first.state, JobState::Queued.as_str());
    }

    #[tokio::test]
    async fn retrying_job_is_hidden_until_its_backoff_elapses() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_retrying(&db, j.id, "file", "kaputt", 5_000).await.unwrap();
        assert!(next_due_job(&db, 4_999).await.unwrap().is_none());
        let due = next_due_job(&db, 5_000).await.unwrap().unwrap();
        assert_eq!(due.attempts, 1);
    }

    #[tokio::test]
    async fn requeue_does_not_consume_an_attempt() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        requeue_job(&db, j.id).await.unwrap();
        let back = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(back.attempts, 0);
        assert_eq!(back.state, JobState::Queued.as_str());
    }

    #[tokio::test]
    async fn interrupted_printing_job_is_failed_never_retried() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        assert_eq!(recover_interrupted(&db).await.unwrap(), 1);
        let back = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(back.state, JobState::Failed.as_str());
        assert_eq!(back.error_kind.as_deref(), Some("interrupted"));
        assert!(next_due_job(&db, 0).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn dedup_lookups_only_match_completed_jobs() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        assert!(!job_done_for_hash(&db, fid, "h1").await.unwrap());
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();
        assert!(job_done_for_hash(&db, fid, "h1").await.unwrap());
        assert!(job_seen_for_stamp(&db, fid, "a.pdf", 100, 1_700_000_000_000).await.unwrap());
        assert!(!job_seen_for_stamp(&db, fid, "a.pdf", 101, 1_700_000_000_000).await.unwrap());
    }

    #[tokio::test]
    async fn failed_job_records_kind_and_message() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        mark_failed(&db, j.id, "file", "PDF nicht lesbar").await.unwrap();
        let back = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(back.state, JobState::Failed.as_str());
        assert_eq!(back.error_message.as_deref(), Some("PDF nicht lesbar"));
        assert!(back.finished_at.is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test db::jobs`
Expected: FAIL — `cannot find type 'NewJob' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/db/jobs.rs`:

```rust
use crate::db::models::PrintJob;
use crate::db::Db;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Printing,
    Retrying,
    Done,
    Failed,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Printing => "printing",
            JobState::Retrying => "retrying",
            JobState::Done => "done",
            JobState::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewJob {
    pub folder_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub mtime_ms: i64,
    pub sha256: String,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
}

pub async fn enqueue_job(db: &Db, n: &NewJob) -> Result<PrintJob, sqlx::Error> {
    sqlx::query_as::<_, PrintJob>(
        "INSERT INTO print_job
           (folder_id, file_path, file_name, size_bytes, mtime_ms, sha256,
            state, printer_name, copies, duplex, color_mode)
         VALUES (?, ?, ?, ?, ?, ?, 'queued', ?, ?, ?, ?) RETURNING *",
    )
    .bind(n.folder_id).bind(&n.file_path).bind(&n.file_name).bind(n.size_bytes)
    .bind(n.mtime_ms).bind(&n.sha256).bind(&n.printer_name).bind(n.copies)
    .bind(&n.duplex).bind(&n.color_mode)
    .fetch_one(db)
    .await
}

/// FIFO by enqueue time. `now_ms` gates jobs waiting out their retry backoff.
pub async fn next_due_job(db: &Db, now_ms: i64) -> Result<Option<PrintJob>, sqlx::Error> {
    sqlx::query_as::<_, PrintJob>(
        "SELECT * FROM print_job
         WHERE state IN ('queued', 'retrying')
           AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
         ORDER BY enqueued_at, id LIMIT 1",
    )
    .bind(now_ms)
    .fetch_optional(db)
    .await
}

pub async fn get_job(db: &Db, id: i64) -> Result<Option<PrintJob>, sqlx::Error> {
    sqlx::query_as::<_, PrintJob>("SELECT * FROM print_job WHERE id = ?")
        .bind(id).fetch_optional(db).await
}

pub async fn mark_printing(db: &Db, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE print_job SET state = 'printing', started_at = datetime('now') WHERE id = ?",
    ).bind(id).execute(db).await?;
    Ok(())
}

pub async fn mark_done(db: &Db, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE print_job SET state = 'done', finished_at = datetime('now'),
           error_kind = NULL, error_message = NULL, next_attempt_at = NULL
         WHERE id = ?",
    ).bind(id).execute(db).await?;
    Ok(())
}

/// File-level failure that will be retried. Consumes one attempt.
pub async fn mark_retrying(
    db: &Db, id: i64, kind: &str, message: &str, next_attempt_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE print_job SET state = 'retrying', attempts = attempts + 1,
           error_kind = ?, error_message = ?, next_attempt_at = ? WHERE id = ?",
    ).bind(kind).bind(message).bind(next_attempt_at).bind(id).execute(db).await?;
    Ok(())
}

pub async fn mark_failed(
    db: &Db, id: i64, kind: &str, message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE print_job SET state = 'failed', finished_at = datetime('now'),
           error_kind = ?, error_message = ?, next_attempt_at = NULL WHERE id = ?",
    ).bind(kind).bind(message).bind(id).execute(db).await?;
    Ok(())
}

/// Printer-level failure: the job goes back to the queue untouched. No attempt
/// is consumed — a switched-off printer must not burn a job's retries.
pub async fn requeue_job(db: &Db, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE print_job SET state = 'queued', started_at = NULL,
           next_attempt_at = NULL WHERE id = ?",
    ).bind(id).execute(db).await?;
    Ok(())
}

/// Called once at startup. A job left in 'printing' means the app died mid-job;
/// it is failed, never silently reprinted.
pub async fn recover_interrupted(db: &Db) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE print_job SET state = 'failed', error_kind = 'interrupted',
           error_message = 'Beim Druck unterbrochen', finished_at = datetime('now')
         WHERE state = 'printing'",
    ).execute(db).await?;
    Ok(r.rows_affected())
}

pub async fn job_done_for_hash(db: &Db, folder_id: i64, sha256: &str) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM print_job WHERE folder_id = ? AND sha256 = ? AND state = 'done'",
    ).bind(folder_id).bind(sha256).fetch_one(db).await?;
    Ok(n > 0)
}

/// Cheap pre-check so the watcher does not hash every file on every tick.
pub async fn job_seen_for_stamp(
    db: &Db, folder_id: i64, file_name: &str, size_bytes: i64, mtime_ms: i64,
) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM print_job
         WHERE folder_id = ? AND file_name = ? AND size_bytes = ? AND mtime_ms = ?",
    ).bind(folder_id).bind(file_name).bind(size_bytes).bind(mtime_ms)
     .fetch_one(db).await?;
    Ok(n > 0)
}

pub async fn list_jobs(
    db: &Db, only_failed: bool, limit: i64,
) -> Result<Vec<PrintJob>, sqlx::Error> {
    let sql = if only_failed {
        "SELECT * FROM print_job WHERE state = 'failed' ORDER BY id DESC LIMIT ?"
    } else {
        "SELECT * FROM print_job ORDER BY id DESC LIMIT ?"
    };
    sqlx::query_as::<_, PrintJob>(sql).bind(limit).fetch_all(db).await
}
```

Add `pub mod jobs;` to the top of `src-tauri/src/db/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test db::jobs`
Expected: PASS — 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db
git commit -m "feat: add print job ledger with retry and crash recovery semantics"
```

---

### Task 4: Settings key-value accessors

**Files:**
- Create: `src-tauri/src/db/settings.rs`
- Modify: `src-tauri/src/db/mod.rs` (add `pub mod settings;`)

**Interfaces:**
- Consumes: `db::Db`.
- Produces: `db::settings::{get_setting, set_setting, notification_mode, user_paused, sumatra_path, start_minimized, NotificationMode}`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/db/settings.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;

    #[tokio::test]
    async fn defaults_apply_on_empty_db() {
        let db = connect("sqlite::memory:").await.unwrap();
        assert_eq!(notification_mode(&db).await, NotificationMode::All);
        assert!(!user_paused(&db).await);
        assert!(!start_minimized(&db).await);
        assert!(sumatra_path(&db).await.is_none());
    }

    #[tokio::test]
    async fn set_then_get_round_trips_and_upserts() {
        let db = connect("sqlite::memory:").await.unwrap();
        set_setting(&db, "notification_mode", "errors").await.unwrap();
        set_setting(&db, "notification_mode", "off").await.unwrap();
        assert_eq!(notification_mode(&db).await, NotificationMode::Off);
        set_setting(&db, "user_paused", "1").await.unwrap();
        assert!(user_paused(&db).await);
    }

    #[tokio::test]
    async fn unparseable_value_falls_back_to_default() {
        let db = connect("sqlite::memory:").await.unwrap();
        set_setting(&db, "notification_mode", "banana").await.unwrap();
        assert_eq!(notification_mode(&db).await, NotificationMode::All);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test db::settings`
Expected: FAIL — `cannot find function 'notification_mode' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/db/settings.rs`:

```rust
use crate::db::Db;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationMode {
    All,
    Errors,
    Off,
}

impl NotificationMode {
    /// Permissive parse with a safe default, matching the house style.
    pub fn parse(s: &str) -> Self {
        match s {
            "errors" => NotificationMode::Errors,
            "off" => NotificationMode::Off,
            _ => NotificationMode::All,
        }
    }
}

pub async fn get_setting(db: &Db, key: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_setting WHERE key = ?")
        .bind(key).fetch_optional(db).await?;
    Ok(row.map(|r| r.0))
}

pub async fn set_setting(db: &Db, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_setting (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    ).bind(key).bind(value).execute(db).await?;
    Ok(())
}

async fn flag(db: &Db, key: &str) -> bool {
    matches!(get_setting(db, key).await.ok().flatten().as_deref(), Some("1"))
}

pub async fn notification_mode(db: &Db) -> NotificationMode {
    match get_setting(db, "notification_mode").await.ok().flatten() {
        Some(v) => NotificationMode::parse(&v),
        None => NotificationMode::All,
    }
}

pub async fn user_paused(db: &Db) -> bool {
    flag(db, "user_paused").await
}

pub async fn start_minimized(db: &Db) -> bool {
    flag(db, "start_minimized").await
}

pub async fn sumatra_path(db: &Db) -> Option<String> {
    get_setting(db, "sumatra_path").await.ok().flatten().filter(|s| !s.is_empty())
}
```

Add `pub mod settings;` to the top of `src-tauri/src/db/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test db::settings`
Expected: PASS — 3 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db
git commit -m "feat: add key-value settings with typed defaulting accessors"
```

---

### Task 5: PrintBackend trait and fake backend

**Files:**
- Create: `src-tauri/src/print/mod.rs`, `src-tauri/src/print/fake.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod print;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `print::{DuplexMode, ColorMode, PrinterInfo, PrinterCapabilities, PrintRequest, PrintErrorKind, PrintError, PrintBackend}`; `print::fake::FakeBackend`.

The trait is `Send + Sync` because the queue worker calls `print` inside `tokio::task::spawn_blocking`. Implementations must not hold a `Pdfium` handle — it is bound per call inside `print`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/print/fake.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{ColorMode, DuplexMode, PrintBackend, PrintErrorKind, PrintRequest};
    use std::path::PathBuf;

    fn req() -> PrintRequest {
        PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "P".into(),
            copies: 2,
            duplex: DuplexMode::LongEdge,
            color: ColorMode::Mono,
        }
    }

    #[test]
    fn records_every_job_it_is_given() {
        let b = FakeBackend::new(&["P", "Q"]);
        b.print(&req()).unwrap();
        b.print(&req()).unwrap();
        assert_eq!(b.jobs().len(), 2);
        assert_eq!(b.jobs()[0].copies, 2);
    }

    #[test]
    fn lists_printers_and_marks_the_first_as_default() {
        let b = FakeBackend::new(&["P", "Q"]);
        let list = b.list_printers().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].is_default);
        assert!(!list[1].is_default);
    }

    #[test]
    fn can_be_told_to_fail_with_a_chosen_kind() {
        let b = FakeBackend::new(&["P"]);
        b.fail_next(PrintErrorKind::Printer, "Drucker offline");
        let err = b.print(&req()).unwrap_err();
        assert_eq!(err.kind, PrintErrorKind::Printer);
        assert!(b.jobs().is_empty());
        // The failure is one-shot: the next call succeeds again.
        b.print(&req()).unwrap();
        assert_eq!(b.jobs().len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test print::fake`
Expected: FAIL — `file not found for module 'print'`.

- [ ] **Step 3: Write the trait and shared types**

Create `src-tauri/src/print/mod.rs`:

```rust
pub mod fake;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplexMode {
    Simplex,
    LongEdge,
    ShortEdge,
}

impl DuplexMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "long_edge" => DuplexMode::LongEdge,
            "short_edge" => DuplexMode::ShortEdge,
            _ => DuplexMode::Simplex,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            DuplexMode::Simplex => "simplex",
            DuplexMode::LongEdge => "long_edge",
            DuplexMode::ShortEdge => "short_edge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    Color,
    Mono,
}

impl ColorMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "color" => ColorMode::Color,
            _ => ColorMode::Mono,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Color => "color",
            ColorMode::Mono => "mono",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PrinterInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PrinterCapabilities {
    pub duplex: bool,
    pub color: bool,
    pub copies: bool,
}

#[derive(Debug, Clone)]
pub struct PrintRequest {
    pub file: PathBuf,
    pub printer: String,
    pub copies: u32,
    pub duplex: DuplexMode,
    pub color: ColorMode,
}

/// Decides whether one job fails or the whole queue holds. See spec section 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintErrorKind {
    File,
    Printer,
    Config,
}

impl PrintErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PrintErrorKind::File => "file",
            PrintErrorKind::Printer => "printer",
            PrintErrorKind::Config => "config",
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct PrintError {
    pub kind: PrintErrorKind,
    pub message: String,
}

impl PrintError {
    pub fn file(msg: impl Into<String>) -> Self {
        PrintError { kind: PrintErrorKind::File, message: msg.into() }
    }
    pub fn printer(msg: impl Into<String>) -> Self {
        PrintError { kind: PrintErrorKind::Printer, message: msg.into() }
    }
    pub fn config(msg: impl Into<String>) -> Self {
        PrintError { kind: PrintErrorKind::Config, message: msg.into() }
    }
}

/// Implementations must be usable from `spawn_blocking`, so `Send + Sync`.
/// They must not hold a `Pdfium` handle; bind it inside `print`.
pub trait PrintBackend: Send + Sync {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError>;
    fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities, PrintError>;
    fn print(&self, req: &PrintRequest) -> Result<(), PrintError>;
}
```

- [ ] **Step 4: Write the fake backend**

Prepend to `src-tauri/src/print/fake.rs`:

```rust
use crate::print::{
    PrintBackend, PrintError, PrintErrorKind, PrintRequest, PrinterCapabilities, PrinterInfo,
};
use std::sync::Mutex;

/// Test double. Records every accepted job and can be armed to fail once.
pub struct FakeBackend {
    printers: Vec<String>,
    jobs: Mutex<Vec<PrintRequest>>,
    next_failure: Mutex<Option<PrintError>>,
}

impl FakeBackend {
    pub fn new(printers: &[&str]) -> Self {
        FakeBackend {
            printers: printers.iter().map(|s| s.to_string()).collect(),
            jobs: Mutex::new(Vec::new()),
            next_failure: Mutex::new(None),
        }
    }

    pub fn jobs(&self) -> Vec<PrintRequest> {
        self.jobs.lock().unwrap().clone()
    }

    /// Arms a one-shot failure for the next `print` call.
    pub fn fail_next(&self, kind: PrintErrorKind, message: &str) {
        *self.next_failure.lock().unwrap() =
            Some(PrintError { kind, message: message.to_string() });
    }
}

impl PrintBackend for FakeBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        Ok(self
            .printers
            .iter()
            .enumerate()
            .map(|(i, n)| PrinterInfo { name: n.clone(), is_default: i == 0 })
            .collect())
    }

    fn capabilities(&self, _printer: &str) -> Result<PrinterCapabilities, PrintError> {
        Ok(PrinterCapabilities { duplex: true, color: true, copies: true })
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        if let Some(e) = self.next_failure.lock().unwrap().take() {
            return Err(e);
        }
        self.jobs.lock().unwrap().push(req.clone());
        Ok(())
    }
}
```

Add `mod print;` to `src-tauri/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test print::fake`
Expected: PASS — 3 tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/print src-tauri/src/lib.rs
git commit -m "feat: add PrintBackend trait and fake backend for testing"
```

---

### Task 6: macOS CUPS backend

**Files:**
- Create: `src-tauri/src/print/macos_cups.rs`
- Modify: `src-tauri/src/print/mod.rs`

**Interfaces:**
- Consumes: `print::{PrintBackend, PrintRequest, PrintError, PrinterInfo, PrinterCapabilities, DuplexMode, ColorMode}`.
- Produces: `print::macos_cups::{CupsBackend, parse_lpstat, lp_args}`.

The argument-building and `lpstat` parsing are pure functions so they can be unit-tested without invoking `lp`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/print/macos_cups.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{ColorMode, DuplexMode, PrintRequest};
    use std::path::PathBuf;

    #[test]
    fn parses_lpstat_output_and_marks_the_default() {
        let out = "printer Brother_MFC is idle.  enabled since Mon\n\
                   printer HP_LaserJet is idle.  enabled since Mon\n\
                   system default destination: HP_LaserJet\n";
        let list = parse_lpstat(out);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Brother_MFC");
        assert!(!list[0].is_default);
        assert!(list[1].is_default);
    }

    #[test]
    fn returns_no_printers_for_empty_output() {
        assert!(parse_lpstat("").is_empty());
    }

    #[test]
    fn builds_lp_arguments_for_duplex_mono() {
        let req = PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "HP".into(),
            copies: 3,
            duplex: DuplexMode::LongEdge,
            color: ColorMode::Mono,
        };
        assert_eq!(
            lp_args(&req),
            vec![
                "-d", "HP",
                "-n", "3",
                "-o", "sides=two-sided-long-edge",
                "-o", "ColorModel=Gray",
                "/tmp/a.pdf",
            ]
        );
    }

    #[test]
    fn builds_lp_arguments_for_simplex_color() {
        let req = PrintRequest {
            file: PathBuf::from("/tmp/b.png"),
            printer: "P".into(),
            copies: 1,
            duplex: DuplexMode::Simplex,
            color: ColorMode::Color,
        };
        assert_eq!(
            lp_args(&req),
            vec![
                "-d", "P",
                "-n", "1",
                "-o", "sides=one-sided",
                "-o", "ColorModel=RGB",
                "/tmp/b.png",
            ]
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test print::macos_cups`
Expected: FAIL — `cannot find function 'parse_lpstat' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/print/macos_cups.rs`:

```rust
use crate::print::{
    ColorMode, DuplexMode, PrintBackend, PrintError, PrintRequest, PrinterCapabilities,
    PrinterInfo,
};
use std::process::Command;

pub struct CupsBackend;

/// Parses `lpstat -p -d` output. Lines look like:
///   printer NAME is idle.  enabled since ...
///   system default destination: NAME
pub fn parse_lpstat(out: &str) -> Vec<PrinterInfo> {
    let mut printers: Vec<PrinterInfo> = Vec::new();
    let mut default_name: Option<String> = None;

    for line in out.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("printer ") {
            if let Some(name) = rest.split_whitespace().next() {
                printers.push(PrinterInfo { name: name.to_string(), is_default: false });
            }
        } else if let Some(rest) = line.strip_prefix("system default destination: ") {
            default_name = Some(rest.trim().to_string());
        }
    }
    if let Some(d) = default_name {
        for p in printers.iter_mut() {
            p.is_default = p.name == d;
        }
    }
    printers
}

/// Builds the argument vector for `lp`. Pure, so it is unit-testable.
pub fn lp_args(req: &PrintRequest) -> Vec<String> {
    let sides = match req.duplex {
        DuplexMode::Simplex => "sides=one-sided",
        DuplexMode::LongEdge => "sides=two-sided-long-edge",
        DuplexMode::ShortEdge => "sides=two-sided-short-edge",
    };
    let color = match req.color {
        ColorMode::Color => "ColorModel=RGB",
        ColorMode::Mono => "ColorModel=Gray",
    };
    vec![
        "-d".into(), req.printer.clone(),
        "-n".into(), req.copies.to_string(),
        "-o".into(), sides.to_string(),
        "-o".into(), color.to_string(),
        req.file.to_string_lossy().to_string(),
    ]
}

impl PrintBackend for CupsBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        let out = Command::new("lpstat")
            .args(["-p", "-d"])
            .output()
            .map_err(|e| PrintError::config(format!("lpstat nicht ausführbar: {e}")))?;
        Ok(parse_lpstat(&String::from_utf8_lossy(&out.stdout)))
    }

    fn capabilities(&self, _printer: &str) -> Result<PrinterCapabilities, PrintError> {
        // CUPS reports capabilities through PPD options; probing them adds no
        // value on the development platform, so everything is offered.
        Ok(PrinterCapabilities { duplex: true, color: true, copies: true })
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        if !req.file.exists() {
            return Err(PrintError::file(format!(
                "Datei nicht gefunden: {}", req.file.display()
            )));
        }
        let out = Command::new("lp")
            .args(lp_args(req))
            .output()
            .map_err(|e| PrintError::printer(format!("lp nicht ausführbar: {e}")))?;
        if out.status.success() {
            return Ok(());
        }
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        // A missing or stopped destination is a printer-level problem.
        if msg.contains("does not exist") || msg.contains("not accepting") {
            Err(PrintError::printer(format!("Drucker nicht erreichbar: {msg}")))
        } else {
            Err(PrintError::file(format!("Druck fehlgeschlagen: {msg}")))
        }
    }
}
```

Add to `src-tauri/src/print/mod.rs`, below `pub mod fake;`:

```rust
#[cfg(target_os = "macos")]
pub mod macos_cups;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test print::macos_cups`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/print
git commit -m "feat: add macOS CUPS print backend"
```

---

### Task 7: Page fitting arithmetic

**Files:**
- Create: `src-tauri/src/print/layout.rs`
- Modify: `src-tauri/src/print/mod.rs` (add `pub mod layout;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `print::layout::{FitRect, fit_centered, render_dpi}`.

This is the part of the Windows printing path that *can* be tested on macOS: computing where a rendered page bitmap lands inside the printable area. Task 8 consumes it.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/print/layout.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_wider_than_area_is_letterboxed_vertically() {
        // 200x100 source into a 400x400 area -> scaled to 400x200, centered.
        let r = fit_centered(200, 100, 400, 400);
        assert_eq!((r.width, r.height), (400, 200));
        assert_eq!((r.x, r.y), (0, 100));
    }

    #[test]
    fn image_taller_than_area_is_pillarboxed_horizontally() {
        let r = fit_centered(100, 200, 400, 400);
        assert_eq!((r.width, r.height), (200, 400));
        assert_eq!((r.x, r.y), (100, 0));
    }

    #[test]
    fn exact_aspect_match_fills_the_area() {
        let r = fit_centered(210, 297, 2100, 2970);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 2100, 2970));
    }

    #[test]
    fn degenerate_sizes_do_not_panic_or_divide_by_zero() {
        let r = fit_centered(0, 0, 400, 400);
        assert_eq!((r.width, r.height), (0, 0));
    }

    #[test]
    fn render_dpi_is_capped_to_bound_memory() {
        assert_eq!(render_dpi(600), MAX_RENDER_DPI);
        assert_eq!(render_dpi(200), 200);
        assert_eq!(render_dpi(0), MIN_RENDER_DPI);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test print::layout`
Expected: FAIL — `cannot find function 'fit_centered' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/print/layout.rs`:

```rust
/// Rasterising above this DPI buys nothing and costs a lot of memory:
/// an A4 page at 300 dpi is roughly 26 MB as 24-bit RGB.
pub const MAX_RENDER_DPI: i32 = 300;
pub const MIN_RENDER_DPI: i32 = 72;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Scales `src` to fit inside `area` preserving aspect ratio, centered.
pub fn fit_centered(src_w: i32, src_h: i32, area_w: i32, area_h: i32) -> FitRect {
    if src_w <= 0 || src_h <= 0 || area_w <= 0 || area_h <= 0 {
        return FitRect { x: 0, y: 0, width: 0, height: 0 };
    }
    let scale = f64::min(area_w as f64 / src_w as f64, area_h as f64 / src_h as f64);
    let width = (src_w as f64 * scale).round() as i32;
    let height = (src_h as f64 * scale).round() as i32;
    FitRect {
        x: (area_w - width) / 2,
        y: (area_h - height) / 2,
        width,
        height,
    }
}

/// Clamps a printer's reported DPI into the range we are willing to rasterise.
pub fn render_dpi(device_dpi: i32) -> i32 {
    device_dpi.clamp(MIN_RENDER_DPI, MAX_RENDER_DPI)
}
```

Add `pub mod layout;` to `src-tauri/src/print/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test print::layout`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/print
git commit -m "feat: add page fitting and render DPI clamping"
```

---

### Task 8: Windows GDI backend

**Files:**
- Create: `src-tauri/src/print/windows_gdi.rs`
- Modify: `src-tauri/src/print/mod.rs`
- Modify: `src-tauri/src/print/pdfium.rs` (created here)

**Interfaces:**
- Consumes: `print::layout::{fit_centered, render_dpi}`, `print::{PrintBackend, PrintRequest, PrintError, ...}`.
- Produces: `print::windows_gdi::GdiBackend`; `print::pdfium::{bind_pdfium, render_pages}`.

**This task cannot be verified by tests on macOS.** Its acceptance is a successful cross-compile check plus the manual checklist in Task 17. The reviewer should treat a green `cargo check` as the gate here, not a green `cargo test`.

- [ ] **Step 1: Add the Windows target and verify the toolchain**

```bash
rustup target add x86_64-pc-windows-msvc
```

Expected: the target installs (or reports it is already installed). Note that `cargo check --target x86_64-pc-windows-msvc` performs type checking only; it does not link, which is what makes it usable from macOS.

- [ ] **Step 2: Write the shared pdfium rasteriser**

Create `src-tauri/src/print/pdfium.rs`:

```rust
use crate::print::PrintError;
use pdfium_render::prelude::*;
use std::path::Path;

/// Binds libpdfium from the candidate directories: the working directory (dev
/// and `cargo test`), the executable's own directory, and the two macOS bundle
/// locations. `pdfium_platform_library_name_at_path` yields the OS-correct file
/// name, so no `cfg` is needed here.
pub fn bind_pdfium() -> Result<Pdfium, PrintError> {
    let mut dirs: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from(".")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("../Frameworks"));
            dirs.push(d.join("../Resources"));
        }
    }
    for dir in &dirs {
        let name = Pdfium::pdfium_platform_library_name_at_path(dir);
        if let Ok(b) = Pdfium::bind_to_library(name) {
            return Ok(Pdfium::new(b));
        }
    }
    Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|e| PrintError::config(format!("pdfium nicht ladbar: {e}")))
}

/// Rasterises every page of a PDF at `dpi`. Synchronous on purpose: `Pdfium` is
/// not `Send`, so the handle must never cross an await point.
pub fn render_pages(file: &Path, dpi: i32) -> Result<Vec<image::RgbImage>, PrintError> {
    let pdfium = bind_pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(file, None)
        .map_err(|e| PrintError::file(format!("PDF nicht lesbar: {e}")))?;

    let mut out = Vec::new();
    for page in doc.pages().iter() {
        let width_pt = page.width().value as f64;
        let height_pt = page.height().value as f64;
        let target_w = ((width_pt / 72.0) * dpi as f64).round() as i32;
        let target_h = ((height_pt / 72.0) * dpi as f64).round() as i32;
        let cfg = PdfRenderConfig::new()
            .set_target_width(target_w)
            .set_maximum_height(target_h);
        let bitmap = page
            .render_with_config(&cfg)
            .map_err(|e| PrintError::file(format!("Render-Fehler: {e}")))?;
        out.push(bitmap.as_image().into_rgb8());
    }
    if out.is_empty() {
        return Err(PrintError::file("PDF enthält keine Seiten".to_string()));
    }
    Ok(out)
}

/// Loads a raster image file as a single "page".
pub fn load_image_page(file: &Path) -> Result<Vec<image::RgbImage>, PrintError> {
    let img = image::open(file)
        .map_err(|e| PrintError::file(format!("Bild nicht lesbar: {e}")))?;
    Ok(vec![img.into_rgb8()])
}

/// Dispatches by extension. PDF and images converge on one bitmap pipeline.
pub fn rasterise(file: &Path, dpi: i32) -> Result<Vec<image::RgbImage>, PrintError> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => render_pages(file, dpi),
        "jpg" | "jpeg" | "png" | "tif" | "tiff" => load_image_page(file),
        other => Err(PrintError::file(format!("Dateityp nicht unterstützt: .{other}"))),
    }
}
```

- [ ] **Step 3: Write the GDI backend**

Create `src-tauri/src/print/windows_gdi.rs`:

```rust
use crate::print::layout::{fit_centered, render_dpi};
use crate::print::pdfium::rasterise;
use crate::print::{
    ColorMode, DuplexMode, PrintBackend, PrintError, PrintRequest, PrinterCapabilities,
    PrinterInfo,
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DEVMODEW,
    DIB_RGB_COLORS, GetDeviceCaps, HORZRES, LOGPIXELSX, PHYSICALHEIGHT, PHYSICALOFFSETX,
    PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY, VERTRES,
};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, DeviceCapabilitiesW, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW,
    OpenPrinterW, DC_COLORDEVICE, DC_COPIES, DC_DUPLEX, DM_COLOR, DM_COPIES, DM_DUPLEX,
    DM_IN_BUFFER, DM_OUT_BUFFER, DMCOLOR_COLOR, DMCOLOR_MONOCHROME, DMDUP_HORIZONTAL,
    DMDUP_SIMPLEX, DMDUP_VERTICAL, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL,
    PRINTER_INFO_4W,
};
use windows::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

pub struct GdiBackend;

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn default_printer_name() -> Option<String> {
    unsafe {
        let mut len: u32 = 0;
        let _ = GetDefaultPrinterW(None, &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        GetDefaultPrinterW(Some(windows::core::PWSTR(buf.as_mut_ptr())), &mut len).ok()?;
        Some(String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize]))
    }
}

impl GdiBackend {
    /// Opens a device context configured with the request's DEVMODE settings.
    /// Returns the DC and the effective page geometry in device pixels.
    unsafe fn open_dc(req: &PrintRequest) -> Result<(windows::Win32::Graphics::Gdi::HDC, i32), PrintError> {
        let name = wide(&req.printer);
        let mut handle = HANDLE::default();
        OpenPrinterW(PCWSTR(name.as_ptr()), &mut handle, None)
            .map_err(|e| PrintError::printer(format!("Drucker nicht erreichbar: {e}")))?;

        // Ask for the driver's DEVMODE size, then fetch and patch it.
        let size = DocumentPropertiesW(None, handle, PCWSTR(name.as_ptr()), None, None, 0);
        if size <= 0 {
            let _ = ClosePrinter(handle);
            return Err(PrintError::printer("Treiber liefert keine Einstellungen".to_string()));
        }
        let mut buf = vec![0u8; size as usize];
        let dm = buf.as_mut_ptr() as *mut DEVMODEW;
        let r = DocumentPropertiesW(
            None, handle, PCWSTR(name.as_ptr()), Some(dm), None, DM_OUT_BUFFER.0 as u32,
        );
        if r < 0 {
            let _ = ClosePrinter(handle);
            return Err(PrintError::printer("Treibereinstellungen nicht lesbar".to_string()));
        }

        (*dm).dmFields |= DM_COPIES | DM_DUPLEX | DM_COLOR;
        (*dm).Anonymous1.Anonymous1.dmCopies = req.copies.min(i16::MAX as u32) as i16;
        (*dm).Anonymous1.Anonymous1.dmDuplex = match req.duplex {
            DuplexMode::Simplex => DMDUP_SIMPLEX,
            DuplexMode::LongEdge => DMDUP_VERTICAL,
            DuplexMode::ShortEdge => DMDUP_HORIZONTAL,
        };
        (*dm).Anonymous1.Anonymous1.dmColor = match req.color {
            ColorMode::Color => DMCOLOR_COLOR,
            ColorMode::Mono => DMCOLOR_MONOCHROME,
        };
        // Let the driver validate and normalise the patched DEVMODE.
        let _ = DocumentPropertiesW(
            None, handle, PCWSTR(name.as_ptr()), Some(dm), Some(dm),
            (DM_IN_BUFFER.0 | DM_OUT_BUFFER.0) as u32,
        );
        let _ = ClosePrinter(handle);

        let hdc = CreateDCW(None, PCWSTR(name.as_ptr()), None, Some(dm));
        if hdc.is_invalid() {
            return Err(PrintError::printer("Kein Gerätekontext für den Drucker".to_string()));
        }
        let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
        Ok((hdc, dpi))
    }
}

impl PrintBackend for GdiBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let mut needed: u32 = 0;
            let mut returned: u32 = 0;
            let _ = EnumPrintersW(flags, None, 4, None, &mut needed, &mut returned);
            if needed == 0 {
                return Ok(Vec::new());
            }
            let mut buf = vec![0u8; needed as usize];
            EnumPrintersW(flags, None, 4, Some(&mut buf), &mut needed, &mut returned)
                .map_err(|e| PrintError::config(format!("Druckerliste nicht lesbar: {e}")))?;

            let items = std::slice::from_raw_parts(
                buf.as_ptr() as *const PRINTER_INFO_4W, returned as usize,
            );
            let default = default_printer_name();
            Ok(items
                .iter()
                .map(|p| {
                    let name = p.pPrinterName.to_string().unwrap_or_default();
                    let is_default = default.as_deref() == Some(name.as_str());
                    PrinterInfo { name, is_default }
                })
                .collect())
        }
    }

    fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities, PrintError> {
        unsafe {
            let name = wide(printer);
            let cap = |what| {
                DeviceCapabilitiesW(PCWSTR(name.as_ptr()), PCWSTR::null(), what, None, None)
            };
            Ok(PrinterCapabilities {
                duplex: cap(DC_DUPLEX) == 1,
                color: cap(DC_COLORDEVICE) == 1,
                copies: cap(DC_COPIES) > 1,
            })
        }
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        if !req.file.exists() {
            return Err(PrintError::file(format!(
                "Datei nicht gefunden: {}", req.file.display()
            )));
        }
        unsafe {
            let (hdc, device_dpi) = Self::open_dc(req)?;
            // Every early return past this point must delete the DC.
            let result = (|| -> Result<(), PrintError> {
                let pages = rasterise(&req.file, render_dpi(device_dpi))?;

                let phys_w = GetDeviceCaps(hdc, PHYSICALWIDTH);
                let phys_h = GetDeviceCaps(hdc, PHYSICALHEIGHT);
                let off_x = GetDeviceCaps(hdc, PHYSICALOFFSETX);
                let off_y = GetDeviceCaps(hdc, PHYSICALOFFSETY);
                let area_w = GetDeviceCaps(hdc, HORZRES).min(phys_w - off_x);
                let area_h = GetDeviceCaps(hdc, VERTRES).min(phys_h - off_y);

                let doc_name = wide(
                    req.file.file_name().and_then(|s| s.to_str()).unwrap_or("Printy"),
                );
                let mut di = DOCINFOW {
                    cbSize: std::mem::size_of::<DOCINFOW>() as i32,
                    lpszDocName: PCWSTR(doc_name.as_ptr()),
                    ..Default::default()
                };
                if StartDocW(hdc, &mut di) <= 0 {
                    return Err(PrintError::printer("Druckauftrag abgelehnt".to_string()));
                }

                for page in &pages {
                    if StartPage(hdc) <= 0 {
                        let _ = EndDoc(hdc);
                        return Err(PrintError::printer("Seite abgelehnt".to_string()));
                    }
                    let (w, h) = (page.width() as i32, page.height() as i32);
                    let rect = fit_centered(w, h, area_w, area_h);

                    // GDI expects bottom-up BGR rows; negative height flips it.
                    let mut bgr = Vec::with_capacity((w * h * 3) as usize);
                    for px in page.pixels() {
                        bgr.extend_from_slice(&[px[2], px[1], px[0]]);
                    }
                    let bi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: w,
                            biHeight: -h,
                            biPlanes: 1,
                            biBitCount: 24,
                            biCompression: BI_RGB.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    StretchDIBits(
                        hdc, rect.x, rect.y, rect.width, rect.height,
                        0, 0, w, h,
                        Some(bgr.as_ptr() as *const _), &bi, DIB_RGB_COLORS, SRCCOPY,
                    );
                    let _ = EndPage(hdc);
                }
                let _ = EndDoc(hdc);
                Ok(())
            })();
            let _ = DeleteDC(hdc);
            result
        }
    }
}
```

Add to `src-tauri/src/print/mod.rs`:

```rust
pub mod layout;
pub mod pdfium;

#[cfg(target_os = "windows")]
pub mod windows_gdi;
```

- [ ] **Step 4: Verify it type-checks for Windows**

Run: `cd src-tauri && cargo check --target x86_64-pc-windows-msvc`
Expected: PASS with no errors. If the `windows` crate's exact item paths or the
`DEVMODEW` union field names differ in the pinned version, fix the imports to
match `cargo doc -p windows --open` rather than guessing — the compiler names the
correct module in its error.

Then confirm nothing broke on the host: `cargo test`
Expected: PASS — all previous tests still green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/print
git commit -m "feat: add Windows GDI print backend with pdfium rasterisation"
```

---

### Task 9: SumatraPDF fallback and backend factory

**Files:**
- Create: `src-tauri/src/print/sumatra.rs`
- Create: `src-tauri/src/print/factory.rs`
- Modify: `src-tauri/src/print/mod.rs`

**Interfaces:**
- Consumes: everything from Tasks 5–8.
- Produces: `print::sumatra::{SumatraBackend, sumatra_settings, find_sumatra}`; `print::factory::backend(sumatra_path: Option<String>) -> Box<dyn PrintBackend>`.

Sumatra is **never bundled** — the app only calls an independently installed copy, which is what keeps GPLv3 obligations off the installer.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/print/sumatra.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{ColorMode, DuplexMode, PrintRequest};
    use std::path::PathBuf;

    fn req(copies: u32, duplex: DuplexMode, color: ColorMode) -> PrintRequest {
        PrintRequest {
            file: PathBuf::from("/tmp/a.pdf"),
            printer: "HP".into(),
            copies,
            duplex,
            color,
        }
    }

    #[test]
    fn builds_settings_string_for_duplex_mono_multiple_copies() {
        let s = sumatra_settings(&req(3, DuplexMode::LongEdge, ColorMode::Mono));
        assert_eq!(s, "3x,duplexlong,monochrome");
    }

    #[test]
    fn omits_copies_when_only_one_is_requested() {
        let s = sumatra_settings(&req(1, DuplexMode::Simplex, ColorMode::Color));
        assert_eq!(s, "simplex,color");
    }

    #[test]
    fn short_edge_maps_to_duplexshort() {
        let s = sumatra_settings(&req(1, DuplexMode::ShortEdge, ColorMode::Color));
        assert_eq!(s, "duplexshort,color");
    }

    #[test]
    fn configured_path_wins_over_the_standard_locations() {
        let here = std::env::current_exe().unwrap();
        let found = find_sumatra(Some(here.to_string_lossy().to_string()));
        assert_eq!(found, Some(here));
    }

    #[test]
    fn a_configured_path_that_does_not_exist_is_ignored() {
        assert!(find_sumatra(Some("/nope/SumatraPDF.exe".into())).is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test print::sumatra`
Expected: FAIL — `cannot find function 'sumatra_settings' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/print/sumatra.rs`:

```rust
use crate::print::{
    ColorMode, DuplexMode, PrintBackend, PrintError, PrintRequest, PrinterCapabilities,
    PrinterInfo,
};
use std::path::PathBuf;
use std::process::Command;

/// Standard install locations, checked only when no path is configured.
const STANDARD_PATHS: &[&str] = &[
    r"C:\Program Files\SumatraPDF\SumatraPDF.exe",
    r"C:\Program Files (x86)\SumatraPDF\SumatraPDF.exe",
];

/// SumatraPDF is never bundled — doing so would attach GPLv3 obligations to the
/// installer. Only an independently installed copy is used.
pub fn find_sumatra(configured: Option<String>) -> Option<PathBuf> {
    if let Some(p) = configured {
        let p = PathBuf::from(p);
        return if p.exists() { Some(p) } else { None };
    }
    STANDARD_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

/// Builds the `-print-settings` value.
pub fn sumatra_settings(req: &PrintRequest) -> String {
    let mut parts: Vec<String> = Vec::new();
    if req.copies > 1 {
        parts.push(format!("{}x", req.copies));
    }
    parts.push(
        match req.duplex {
            DuplexMode::Simplex => "simplex",
            DuplexMode::LongEdge => "duplexlong",
            DuplexMode::ShortEdge => "duplexshort",
        }
        .to_string(),
    );
    parts.push(
        match req.color {
            ColorMode::Color => "color",
            ColorMode::Mono => "monochrome",
        }
        .to_string(),
    );
    parts.join(",")
}

pub struct SumatraBackend {
    pub exe: PathBuf,
}

impl PrintBackend for SumatraBackend {
    fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
        // Sumatra is a fallback for rendering only; it never enumerates.
        Err(PrintError::config(
            "SumatraPDF liefert keine Druckerliste".to_string(),
        ))
    }

    fn capabilities(&self, _printer: &str) -> Result<PrinterCapabilities, PrintError> {
        Ok(PrinterCapabilities { duplex: true, color: true, copies: true })
    }

    fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
        let out = Command::new(&self.exe)
            .arg("-print-to").arg(&req.printer)
            .arg("-silent")
            .arg("-print-settings").arg(sumatra_settings(req))
            .arg(&req.file)
            .output()
            .map_err(|e| PrintError::file(format!("SumatraPDF nicht startbar: {e}")))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(PrintError::file(format!(
                "SumatraPDF-Druck fehlgeschlagen (Code {:?})",
                out.status.code()
            )))
        }
    }
}
```

- [ ] **Step 4: Write the factory**

Create `src-tauri/src/print/factory.rs`:

```rust
use crate::print::PrintBackend;

/// Selects the platform backend. On Windows the GDI backend is primary and
/// SumatraPDF, if installed, is consulted only for rendering failures — inside
/// the same attempt, never for printer-level errors.
#[cfg(target_os = "windows")]
pub fn backend(sumatra_path: Option<String>) -> Box<dyn PrintBackend> {
    use crate::print::sumatra::{find_sumatra, SumatraBackend};
    use crate::print::windows_gdi::GdiBackend;
    use crate::print::{PrintError, PrintErrorKind, PrintRequest, PrinterCapabilities, PrinterInfo};

    struct WithFallback {
        primary: GdiBackend,
        fallback: Option<SumatraBackend>,
    }

    impl PrintBackend for WithFallback {
        fn list_printers(&self) -> Result<Vec<PrinterInfo>, PrintError> {
            self.primary.list_printers()
        }
        fn capabilities(&self, printer: &str) -> Result<PrinterCapabilities, PrintError> {
            self.primary.capabilities(printer)
        }
        fn print(&self, req: &PrintRequest) -> Result<(), PrintError> {
            match self.primary.print(req) {
                Ok(()) => Ok(()),
                // Printer-level problems are not rendering problems.
                Err(e) if e.kind == PrintErrorKind::Printer => Err(e),
                Err(e) => match &self.fallback {
                    Some(f) => f.print(req),
                    None => Err(e),
                },
            }
        }
    }

    Box::new(WithFallback {
        primary: GdiBackend,
        fallback: find_sumatra(sumatra_path).map(|exe| SumatraBackend { exe }),
    })
}

#[cfg(target_os = "macos")]
pub fn backend(_sumatra_path: Option<String>) -> Box<dyn PrintBackend> {
    Box::new(crate::print::macos_cups::CupsBackend)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn backend(_sumatra_path: Option<String>) -> Box<dyn PrintBackend> {
    Box::new(crate::print::fake::FakeBackend::new(&[]))
}
```

Add to `src-tauri/src/print/mod.rs` — the module is **not** `cfg`-gated, so
`sumatra_settings` and `find_sumatra` and their tests compile and run on macOS:

```rust
pub mod factory;
pub mod sumatra;
```

Only the backend struct and its `impl` are Windows-only. Apply these two
attributes in `sumatra.rs`, leaving the two free functions ungated:

```rust
#[cfg(target_os = "windows")]
pub struct SumatraBackend {
    pub exe: PathBuf,
}

#[cfg(target_os = "windows")]
impl PrintBackend for SumatraBackend {
    // body exactly as written in the code block above
}
```

On macOS this makes the `PrintBackend`, `PrinterCapabilities`, `PrinterInfo` and
`Command` imports unused; narrow the import list to what the free functions need
(`ColorMode`, `DuplexMode`, `PrintRequest`, `PathBuf`) and move the rest inside a
`#[cfg(target_os = "windows")]` `use` block so the build stays warning-free.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test print::sumatra`
Expected: PASS — 5 tests.

Run: `cd src-tauri && cargo check --target x86_64-pc-windows-msvc`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/print
git commit -m "feat: add SumatraPDF fallback and platform backend factory"
```

---

### Task 10: Folder scanning with reserved-directory exclusion

**Files:**
- Create: `src-tauri/src/intake/mod.rs`, `src-tauri/src/intake/scan.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod intake;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `intake::scan::{PRINTED_DIR, FAILED_DIR, scan_folder}`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/intake/scan.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("printy_scan_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn returns_only_files_with_configured_extensions() {
        let d = temp("ext");
        fs::write(d.join("a.pdf"), b"x").unwrap();
        fs::write(d.join("b.PDF"), b"x").unwrap();
        fs::write(d.join("c.txt"), b"x").unwrap();
        let mut got: Vec<String> = scan_folder(&d, &["pdf".into()])
            .unwrap().iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        got.sort();
        assert_eq!(got, vec!["a.pdf", "b.PDF"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn never_descends_into_printed_or_failed() {
        let d = temp("reserved");
        fs::create_dir_all(d.join(PRINTED_DIR)).unwrap();
        fs::create_dir_all(d.join(FAILED_DIR)).unwrap();
        fs::write(d.join(PRINTED_DIR).join("old.pdf"), b"x").unwrap();
        fs::write(d.join(FAILED_DIR).join("bad.pdf"), b"x").unwrap();
        fs::write(d.join("new.pdf"), b"x").unwrap();
        let got = scan_folder(&d, &["pdf".into()]).unwrap();
        assert_eq!(got.len(), 1, "printed/ and failed/ must never be rescanned");
        assert!(got[0].ends_with("new.pdf"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ignores_subdirectories_because_scanning_is_not_recursive() {
        let d = temp("norecurse");
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::write(d.join("sub").join("deep.pdf"), b"x").unwrap();
        assert!(scan_folder(&d, &["pdf".into()]).unwrap().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_folder_is_an_error_not_an_empty_list() {
        let d = std::env::temp_dir().join("printy_scan_absent_does_not_exist");
        let _ = std::fs::remove_dir_all(&d);
        assert!(scan_folder(&d, &["pdf".into()]).is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test intake::scan`
Expected: FAIL — `file not found for module 'intake'`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/intake/scan.rs`:

```rust
use std::path::{Path, PathBuf};

/// Reserved subdirectory names. They are never scanned — otherwise the watcher
/// rediscovers the file it just moved and prints in a loop.
pub const PRINTED_DIR: &str = "printed";
pub const FAILED_DIR: &str = "failed";

/// Lists printable candidates in `root`. Non-recursive by design.
pub fn scan_folder(root: &Path, types: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let wanted: Vec<String> = types.iter().map(|t| t.to_ascii_lowercase()).collect();
    let mut out = Vec::new();

    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if wanted.iter().any(|w| *w == ext) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}
```

Create `src-tauri/src/intake/mod.rs`:

```rust
pub mod scan;
```

Add `mod intake;` to `src-tauri/src/lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test intake::scan`
Expected: PASS — 4 tests. Note the reserved-directory test passes because
scanning is non-recursive; the constants exist so Task 12 writes into those
directories and Task 10's guarantee stays explicit.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/intake src-tauri/src/lib.rs
git commit -m "feat: add non-recursive folder scanning with type filtering"
```

---

### Task 11: Stability check

**Files:**
- Create: `src-tauri/src/intake/stability.rs`
- Modify: `src-tauri/src/intake/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `intake::stability::{FileStamp, stamp, StabilityTracker}`.

This is the check that stops Printy printing a PDF a scanner is still writing.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/intake/stability.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("printy_stab_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_file_is_not_stable_on_first_sight() {
        let d = temp("first");
        let f = d.join("a.pdf");
        fs::write(&f, b"12345").unwrap();
        let mut t = StabilityTracker::new();
        assert!(!t.observe(&f, stamp(&f).unwrap()));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_is_stable_on_the_second_identical_observation() {
        let d = temp("second");
        let f = d.join("a.pdf");
        fs::write(&f, b"12345").unwrap();
        let s = stamp(&f).unwrap();
        let mut t = StabilityTracker::new();
        t.observe(&f, s.clone());
        assert!(t.observe(&f, s));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_growing_file_never_becomes_stable() {
        let d = temp("growing");
        let f = d.join("a.pdf");
        let mut t = StabilityTracker::new();
        for i in 1..5 {
            fs::write(&f, vec![b'x'; i * 100]).unwrap();
            let s = FileStamp { size: (i * 100) as u64, mtime_ms: 1_000 + i as i64 };
            assert!(!t.observe(&f, s), "a file still being written must not print");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn same_size_but_newer_mtime_resets_stability() {
        let f = PathBuf::from("/tmp/printy-virtual.pdf");
        let mut t = StabilityTracker::new();
        t.observe(&f, FileStamp { size: 100, mtime_ms: 1_000 });
        assert!(!t.observe(&f, FileStamp { size: 100, mtime_ms: 2_000 }));
        assert!(t.observe(&f, FileStamp { size: 100, mtime_ms: 2_000 }));
    }

    #[test]
    fn forget_drops_tracking_state_for_a_path() {
        let f = PathBuf::from("/tmp/printy-virtual2.pdf");
        let s = FileStamp { size: 1, mtime_ms: 1 };
        let mut t = StabilityTracker::new();
        t.observe(&f, s.clone());
        t.forget(&f);
        assert!(!t.observe(&f, s), "after forget the file starts over");
    }

    #[test]
    fn stamp_reads_size_and_mtime_from_disk() {
        let d = temp("stamp");
        let f = d.join("a.pdf");
        fs::write(&f, b"hello").unwrap();
        let s = stamp(&f).unwrap();
        assert_eq!(s.size, 5);
        assert!(s.mtime_ms > 0);
        let _ = fs::remove_dir_all(&d);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test intake::stability`
Expected: FAIL — `cannot find type 'StabilityTracker' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/intake/stability.rs`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub mtime_ms: i64,
}

pub fn stamp(path: &Path) -> std::io::Result<FileStamp> {
    let md = std::fs::metadata(path)?;
    let mtime_ms = md
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(FileStamp { size: md.len(), mtime_ms })
}

/// Remembers the previous stamp per path. A file counts as complete only when
/// two consecutive observations are identical — the cheapest reliable way to
/// avoid printing a file that is still being written.
#[derive(Debug, Default)]
pub struct StabilityTracker {
    seen: HashMap<PathBuf, FileStamp>,
}

impl StabilityTracker {
    pub fn new() -> Self {
        StabilityTracker { seen: HashMap::new() }
    }

    /// Returns true when this stamp matches the one recorded on the last tick.
    pub fn observe(&mut self, path: &Path, current: FileStamp) -> bool {
        match self.seen.insert(path.to_path_buf(), current.clone()) {
            Some(prev) => prev == current,
            None => false,
        }
    }

    pub fn forget(&mut self, path: &Path) {
        self.seen.remove(path);
    }
}

/// A file that cannot be opened for reading is still held by its writer.
pub fn is_readable(path: &Path) -> bool {
    std::fs::File::open(path).is_ok()
}
```

Add `pub mod stability;` to `src-tauri/src/intake/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test intake::stability`
Expected: PASS — 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/intake
git commit -m "feat: add two-tick file stability check"
```

---

### Task 12: Post-print actions

**Files:**
- Create: `src-tauri/src/intake/post.rs`
- Modify: `src-tauri/src/intake/mod.rs`

**Interfaces:**
- Consumes: `intake::scan::{PRINTED_DIR, FAILED_DIR}`.
- Produces: `intake::post::{PostAction, unique_target, apply_success, apply_failure}`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/intake/post.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("printy_post_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn move_relocates_the_file_into_printed() {
        let d = temp("move");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_success(&f, &d, PostAction::Move, 1_700_000_000_000).unwrap();
        assert!(!f.exists());
        assert!(d.join(PRINTED_DIR).join("a.pdf").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn move_with_a_name_collision_appends_a_timestamp_and_never_overwrites() {
        let d = temp("collide");
        fs::create_dir_all(d.join(PRINTED_DIR)).unwrap();
        fs::write(d.join(PRINTED_DIR).join("a.pdf"), b"first").unwrap();
        let f = d.join("a.pdf");
        fs::write(&f, b"second").unwrap();

        apply_success(&f, &d, PostAction::Move, 1_700_000_000_000).unwrap();

        assert_eq!(
            fs::read(d.join(PRINTED_DIR).join("a.pdf")).unwrap(),
            b"first",
            "the existing file must survive untouched"
        );
        let n = fs::read_dir(d.join(PRINTED_DIR)).unwrap().count();
        assert_eq!(n, 2, "the new file lands beside it under a different name");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn keep_leaves_the_file_exactly_where_it_was() {
        let d = temp("keep");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_success(&f, &d, PostAction::Keep, 0).unwrap();
        assert!(f.exists());
        assert!(!d.join(PRINTED_DIR).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn delete_removes_the_file() {
        let d = temp("delete");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_success(&f, &d, PostAction::Delete, 0).unwrap();
        assert!(!f.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn failure_moves_to_failed_only_in_move_mode() {
        let d = temp("failed");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_failure(&f, &d, PostAction::Move, 0).unwrap();
        assert!(d.join(FAILED_DIR).join("a.pdf").exists());

        let g = d.join("b.pdf");
        fs::write(&g, b"x").unwrap();
        apply_failure(&g, &d, PostAction::Delete, 0).unwrap();
        assert!(g.exists(), "a failed job is never deleted");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn unique_target_is_stable_when_there_is_no_collision() {
        let d = temp("unique");
        let t = unique_target(&d, "a.pdf", 1_700_000_000_000);
        assert_eq!(t, d.join("a.pdf"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn post_action_parses_permissively() {
        assert_eq!(PostAction::parse("keep"), PostAction::Keep);
        assert_eq!(PostAction::parse("delete"), PostAction::Delete);
        assert_eq!(PostAction::parse("banana"), PostAction::Move);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test intake::post`
Expected: FAIL — `cannot find type 'PostAction' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/intake/post.rs`:

```rust
use crate::intake::scan::{FAILED_DIR, PRINTED_DIR};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAction {
    Move,
    Keep,
    Delete,
}

impl PostAction {
    pub fn parse(s: &str) -> Self {
        match s {
            "keep" => PostAction::Keep,
            "delete" => PostAction::Delete,
            _ => PostAction::Move,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PostAction::Move => "move",
            PostAction::Keep => "keep",
            PostAction::Delete => "delete",
        }
    }
}

/// Returns a free path in `dir` for `file_name`. On collision a timestamp is
/// inserted before the extension; the existing file is never overwritten.
/// `now_ms` is injected so the behaviour is testable.
pub fn unique_target(dir: &Path, file_name: &str, now_ms: i64) -> PathBuf {
    let direct = dir.join(file_name);
    if !direct.exists() {
        return direct;
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("datei");
    let ext = path.extension().and_then(|s| s.to_str());

    let mut n = 0;
    loop {
        let suffix = if n == 0 { format!("{now_ms}") } else { format!("{now_ms}-{n}") };
        let candidate = match ext {
            Some(e) => dir.join(format!("{stem}-{suffix}.{e}")),
            None => dir.join(format!("{stem}-{suffix}")),
        };
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

fn move_into(file: &Path, root: &Path, sub: &str, now_ms: i64) -> std::io::Result<PathBuf> {
    let dir = root.join(sub);
    std::fs::create_dir_all(&dir)?;
    let name = file.file_name().and_then(|s| s.to_str()).unwrap_or("datei");
    let target = unique_target(&dir, name, now_ms);
    std::fs::rename(file, &target)?;
    Ok(target)
}

/// Applies the folder's rule after a successful print.
pub fn apply_success(
    file: &Path, root: &Path, action: PostAction, now_ms: i64,
) -> std::io::Result<()> {
    match action {
        PostAction::Move => {
            move_into(file, root, PRINTED_DIR, now_ms)?;
        }
        PostAction::Keep => {}
        PostAction::Delete => std::fs::remove_file(file)?,
    }
    Ok(())
}

/// Applies the folder's rule after a job failed for good. A failed file is
/// never deleted — only moved aside when the folder moves files at all.
pub fn apply_failure(
    file: &Path, root: &Path, action: PostAction, now_ms: i64,
) -> std::io::Result<()> {
    if action == PostAction::Move {
        move_into(file, root, FAILED_DIR, now_ms)?;
    }
    Ok(())
}
```

Add `pub mod post;` to `src-tauri/src/intake/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test intake::post`
Expected: PASS — 7 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/intake
git commit -m "feat: add post-print move, keep and delete rules"
```

---

### Task 13: Content hashing and deduplication decision

**Files:**
- Create: `src-tauri/src/intake/dedup.rs`
- Modify: `src-tauri/src/intake/mod.rs`

**Interfaces:**
- Consumes: `db::{Db, jobs}`, `intake::post::PostAction`.
- Produces: `intake::dedup::{sha256_file, should_enqueue}`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/intake/dedup.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;
    use crate::db::folders::{create_folder, NewFolder};
    use crate::db::jobs::{enqueue_job, mark_done, mark_printing, NewJob};
    use std::fs;

    fn temp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("printy_dedup_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    async fn setup() -> (crate::db::Db, i64) {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: "/tmp/f".into(), poll_interval_secs: 5,
            file_types: vec!["pdf".into()], printer_name: "P".into(), copies: 1,
            duplex: "simplex".into(), color_mode: "mono".into(), post_action: "keep".into(),
        }).await.unwrap();
        (db, f.id)
    }

    #[test]
    fn hashes_file_content_not_file_name() {
        let d = temp("hash");
        fs::write(d.join("a.pdf"), b"same bytes").unwrap();
        fs::write(d.join("b.pdf"), b"same bytes").unwrap();
        fs::write(d.join("c.pdf"), b"other bytes").unwrap();
        let a = sha256_file(&d.join("a.pdf")).unwrap();
        let b = sha256_file(&d.join("b.pdf")).unwrap();
        let c = sha256_file(&d.join("c.pdf")).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn keep_mode_skips_a_file_whose_content_was_already_printed() {
        let (db, fid) = setup().await;
        let d = temp("keepmode");
        let f = d.join("a.pdf");
        fs::write(&f, b"payload").unwrap();
        let hash = sha256_file(&f).unwrap();

        assert!(should_enqueue(&db, fid, &f, PostAction::Keep, &hash).await.unwrap());

        let j = enqueue_job(&db, &NewJob {
            folder_id: fid, file_path: f.to_string_lossy().into(), file_name: "a.pdf".into(),
            size_bytes: 7, mtime_ms: 1, sha256: hash.clone(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
        }).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();

        assert!(!should_enqueue(&db, fid, &f, PostAction::Keep, &hash).await.unwrap());
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn identical_content_under_a_new_name_is_still_a_duplicate_in_keep_mode() {
        let (db, fid) = setup().await;
        let d = temp("rename");
        fs::write(d.join("a.pdf"), b"payload").unwrap();
        fs::write(d.join("copy.pdf"), b"payload").unwrap();
        let hash = sha256_file(&d.join("a.pdf")).unwrap();

        let j = enqueue_job(&db, &NewJob {
            folder_id: fid, file_path: "/tmp/f/a.pdf".into(), file_name: "a.pdf".into(),
            size_bytes: 7, mtime_ms: 1, sha256: hash.clone(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
        }).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();

        let other = sha256_file(&d.join("copy.pdf")).unwrap();
        assert!(!should_enqueue(&db, fid, &d.join("copy.pdf"), PostAction::Keep, &other)
            .await.unwrap());
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn move_mode_does_not_consult_the_hash_ledger() {
        let (db, fid) = setup().await;
        let d = temp("movemode");
        let f = d.join("a.pdf");
        fs::write(&f, b"payload").unwrap();
        let hash = sha256_file(&f).unwrap();

        let j = enqueue_job(&db, &NewJob {
            folder_id: fid, file_path: f.to_string_lossy().into(), file_name: "a.pdf".into(),
            size_bytes: 7, mtime_ms: 1, sha256: hash.clone(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
        }).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();

        // The file left the folder in move mode, so a file sitting there again
        // is genuinely new work and must print.
        assert!(should_enqueue(&db, fid, &f, PostAction::Move, &hash).await.unwrap());
        let _ = fs::remove_dir_all(&d);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test intake::dedup`
Expected: FAIL — `cannot find function 'sha256_file' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/intake/dedup.rs`:

```rust
use crate::db::{jobs, Db};
use crate::intake::post::PostAction;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Streams the file so a large PDF does not land in memory twice.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Decides whether a stable candidate should become a job.
///
/// In `move` and `delete` mode the file leaves the folder after printing, so a
/// file present now is new work by definition. Only `keep` mode needs the
/// content ledger, because there the printed file stays put and would otherwise
/// be rediscovered on every tick.
pub async fn should_enqueue(
    db: &Db,
    folder_id: i64,
    _file: &Path,
    action: PostAction,
    sha256: &str,
) -> Result<bool, sqlx::Error> {
    match action {
        PostAction::Keep => Ok(!jobs::job_done_for_hash(db, folder_id, sha256).await?),
        PostAction::Move | PostAction::Delete => Ok(true),
    }
}
```

Add `pub mod dedup;` to `src-tauri/src/intake/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test intake::dedup`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/intake
git commit -m "feat: add content hashing and mode-aware deduplication"
```

---

### Task 14: Queue worker with retry, backoff and printer hold

**Files:**
- Create: `src-tauri/src/queue/mod.rs`, `src-tauri/src/queue/worker.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod queue;`)

**Interfaces:**
- Consumes: `db::{Db, jobs, folders}`, `print::{PrintBackend, PrintRequest, PrintErrorKind, DuplexMode, ColorMode}`, `intake::post::{PostAction, apply_success, apply_failure}`.
- Produces: `queue::{MAX_ATTEMPTS, BACKOFF_SECS, backoff_ms}`; `queue::worker::{QueueOutcome, run_one}`.

`run_one` is a single, fully testable step of the worker: pick the next due job, print it, apply the rule, record the outcome. The Tokio loop around it is added in Task 15.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/queue/worker.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;
    use crate::db::folders::{create_folder, NewFolder};
    use crate::db::jobs::{enqueue_job, get_job, JobState, NewJob};
    use crate::print::fake::FakeBackend;
    use crate::print::PrintErrorKind;
    use std::fs;

    fn temp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("printy_queue_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    async fn setup(dir: &std::path::Path, action: &str) -> (crate::db::Db, i64) {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: dir.to_string_lossy().into(), poll_interval_secs: 5,
            file_types: vec!["pdf".into()], printer_name: "P".into(), copies: 2,
            duplex: "long_edge".into(), color_mode: "mono".into(), post_action: action.into(),
        }).await.unwrap();
        (db, f.id)
    }

    async fn queue_file(db: &crate::db::Db, fid: i64, path: &std::path::Path) -> i64 {
        enqueue_job(db, &NewJob {
            folder_id: fid,
            file_path: path.to_string_lossy().into(),
            file_name: path.file_name().unwrap().to_string_lossy().into(),
            size_bytes: 1, mtime_ms: 1, sha256: "h".into(),
            printer_name: "P".into(), copies: 2,
            duplex: "long_edge".into(), color_mode: "mono".into(),
        }).await.unwrap().id
    }

    #[tokio::test]
    async fn prints_the_job_and_applies_the_move_rule() {
        let d = temp("ok");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        let (db, fid) = setup(&d, "move").await;
        let id = queue_file(&db, fid, &f).await;
        let backend = FakeBackend::new(&["P"]);

        let out = run_one(&db, &backend, 1_000).await.unwrap();
        assert_eq!(out, QueueOutcome::Printed);

        let jobs = backend.jobs();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].copies, 2);
        assert_eq!(jobs[0].duplex, crate::print::DuplexMode::LongEdge);
        assert_eq!(get_job(&db, id).await.unwrap().unwrap().state, JobState::Done.as_str());
        assert!(d.join("printed").join("a.pdf").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn reports_idle_when_nothing_is_due() {
        let d = temp("idle");
        let (db, _) = setup(&d, "move").await;
        let backend = FakeBackend::new(&["P"]);
        assert_eq!(run_one(&db, &backend, 1_000).await.unwrap(), QueueOutcome::Idle);
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn a_file_error_retries_with_the_documented_backoff() {
        let d = temp("fileerr");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        let (db, fid) = setup(&d, "move").await;
        let id = queue_file(&db, fid, &f).await;
        let backend = FakeBackend::new(&["P"]);

        backend.fail_next(PrintErrorKind::File, "kaputt");
        let out = run_one(&db, &backend, 1_000).await.unwrap();
        assert_eq!(out, QueueOutcome::Retried);

        let j = get_job(&db, id).await.unwrap().unwrap();
        assert_eq!(j.attempts, 1);
        assert_eq!(j.next_attempt_at, Some(1_000 + 5_000));
        assert!(f.exists(), "the file stays put until the job fails for good");
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn a_file_error_fails_for_good_after_three_attempts_and_moves_to_failed() {
        let d = temp("exhaust");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        let (db, fid) = setup(&d, "move").await;
        let id = queue_file(&db, fid, &f).await;
        let backend = FakeBackend::new(&["P"]);

        let mut now = 1_000;
        for _ in 0..MAX_ATTEMPTS {
            backend.fail_next(PrintErrorKind::File, "kaputt");
            run_one(&db, &backend, now).await.unwrap();
            now += 1_000_000;
        }

        let j = get_job(&db, id).await.unwrap().unwrap();
        assert_eq!(j.state, JobState::Failed.as_str());
        assert_eq!(j.error_kind.as_deref(), Some("file"));
        assert!(d.join("failed").join("a.pdf").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn a_printer_error_holds_the_queue_without_consuming_an_attempt() {
        let d = temp("printererr");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        let (db, fid) = setup(&d, "move").await;
        let id = queue_file(&db, fid, &f).await;
        let backend = FakeBackend::new(&["P"]);

        backend.fail_next(PrintErrorKind::Printer, "Drucker offline");
        let out = run_one(&db, &backend, 1_000).await.unwrap();
        assert_eq!(out, QueueOutcome::PrinterHold("Drucker offline".into()));

        let j = get_job(&db, id).await.unwrap().unwrap();
        assert_eq!(j.attempts, 0, "a dead printer must not burn the job's retries");
        assert_eq!(j.state, JobState::Queued.as_str());
        assert!(f.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn a_vanished_file_fails_immediately_without_retrying() {
        let d = temp("vanished");
        let (db, fid) = setup(&d, "move").await;
        let id = queue_file(&db, fid, &d.join("gone.pdf")).await;
        let backend = FakeBackend::new(&["P"]);

        let out = run_one(&db, &backend, 1_000).await.unwrap();
        assert_eq!(out, QueueOutcome::Failed);
        let j = get_job(&db, id).await.unwrap().unwrap();
        assert_eq!(j.state, JobState::Failed.as_str());
        assert!(backend.jobs().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn backoff_follows_the_documented_schedule() {
        assert_eq!(backoff_ms(0), 5_000);
        assert_eq!(backoff_ms(1), 30_000);
        assert_eq!(backoff_ms(2), 120_000);
        assert_eq!(backoff_ms(99), 120_000);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test queue::worker`
Expected: FAIL — `file not found for module 'queue'`.

- [ ] **Step 3: Write the queue constants**

Create `src-tauri/src/queue/mod.rs`:

```rust
pub mod worker;

/// Retry schedule from the spec: 5 s, 30 s, 120 s, three attempts total.
pub const MAX_ATTEMPTS: i64 = 3;
pub const BACKOFF_SECS: [i64; 3] = [5, 30, 120];

/// Backoff after `attempts_so_far` failed attempts, in milliseconds.
pub fn backoff_ms(attempts_so_far: i64) -> i64 {
    let idx = attempts_so_far.clamp(0, BACKOFF_SECS.len() as i64 - 1) as usize;
    BACKOFF_SECS[idx] * 1_000
}
```

- [ ] **Step 4: Write the worker step**

Prepend to `src-tauri/src/queue/worker.rs`:

```rust
use crate::db::{folders, jobs, Db};
use crate::intake::post::{apply_failure, apply_success, PostAction};
use crate::print::{ColorMode, DuplexMode, PrintBackend, PrintErrorKind, PrintRequest};
use crate::queue::{backoff_ms, MAX_ATTEMPTS};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueOutcome {
    /// Nothing was due.
    Idle,
    Printed,
    /// File-level failure, will be tried again.
    Retried,
    /// File-level failure, attempts exhausted.
    Failed,
    /// Printer-level failure: the caller must hold the whole queue.
    PrinterHold(String),
}

/// One step of the queue: take the next due job, print it, record the outcome.
/// `now_ms` is injected so retry scheduling is testable without sleeping.
pub async fn run_one<B: PrintBackend + ?Sized>(
    db: &Db,
    backend: &B,
    now_ms: i64,
) -> Result<QueueOutcome, sqlx::Error> {
    let Some(job) = jobs::next_due_job(db, now_ms).await? else {
        return Ok(QueueOutcome::Idle);
    };

    let folder = folders::get_folder(db, job.folder_id).await?;
    let (root, action) = match &folder {
        Some(f) => (PathBuf::from(&f.path), PostAction::parse(&f.post_action)),
        None => (PathBuf::new(), PostAction::Keep),
    };
    let file = PathBuf::from(&job.file_path);

    jobs::mark_printing(db, job.id).await?;

    // A file that disappeared between discovery and printing is not worth three
    // attempts — there is nothing to retry.
    if !file.exists() {
        jobs::mark_failed(db, job.id, "file", "Datei nicht mehr vorhanden").await?;
        return Ok(QueueOutcome::Failed);
    }

    let req = PrintRequest {
        file: file.clone(),
        printer: job.printer_name.clone(),
        copies: job.copies.max(1) as u32,
        duplex: DuplexMode::parse(&job.duplex),
        color: ColorMode::parse(&job.color_mode),
    };

    match backend.print(&req) {
        Ok(()) => {
            // A post-action failure is recorded, but the job is never reprinted.
            match apply_success(&file, &root, action, now_ms) {
                Ok(()) => {
                    jobs::mark_done(db, job.id).await?;
                    Ok(QueueOutcome::Printed)
                }
                Err(e) => {
                    jobs::mark_failed(
                        db, job.id, "file",
                        &format!("Gedruckt, aber Nachbehandlung fehlgeschlagen: {e}"),
                    ).await?;
                    Ok(QueueOutcome::Failed)
                }
            }
        }
        Err(e) if e.kind == PrintErrorKind::Printer => {
            jobs::requeue_job(db, job.id).await?;
            Ok(QueueOutcome::PrinterHold(e.message))
        }
        Err(e) => {
            let kind = e.kind.as_str();
            if job.attempts + 1 >= MAX_ATTEMPTS {
                jobs::mark_failed(db, job.id, kind, &e.message).await?;
                let _ = apply_failure(&file, &root, action, now_ms);
                Ok(QueueOutcome::Failed)
            } else {
                let next = now_ms + backoff_ms(job.attempts);
                jobs::mark_retrying(db, job.id, kind, &e.message, next).await?;
                Ok(QueueOutcome::Retried)
            }
        }
    }
}
```

Add `mod queue;` to `src-tauri/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test queue`
Expected: PASS — 7 tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/queue src-tauri/src/lib.rs
git commit -m "feat: add serial queue step with retry, backoff and printer hold"
```

---

### Task 15: Watcher tick

**Files:**
- Create: `src-tauri/src/watcher/mod.rs`, `src-tauri/src/watcher/tick.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod watcher;`)

**Interfaces:**
- Consumes: `db::{Db, folders, jobs}`, `intake::{scan, stability, dedup, post}`.
- Produces: `watcher::tick::{TickReport, tick_folder}`.

`tick_folder` is the whole per-folder scan as one testable function. The Tokio task around it is added in Task 16.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/watcher/tick.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;
    use crate::db::folders::{create_folder, NewFolder};
    use crate::db::jobs::list_jobs;
    use crate::intake::stability::StabilityTracker;
    use std::fs;

    fn temp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("printy_tick_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    async fn setup(dir: &std::path::Path, action: &str) -> (crate::db::Db, i64) {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: dir.to_string_lossy().into(), poll_interval_secs: 5,
            file_types: vec!["pdf".into()], printer_name: "P".into(), copies: 1,
            duplex: "simplex".into(), color_mode: "mono".into(), post_action: action.into(),
        }).await.unwrap();
        (db, f.id)
    }

    #[tokio::test]
    async fn a_file_is_enqueued_only_on_the_second_tick() {
        let d = temp("second");
        let (db, fid) = setup(&d, "move").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let mut tracker = StabilityTracker::new();
        fs::write(d.join("a.pdf"), b"payload").unwrap();

        let r1 = tick_folder(&db, &folder, &mut tracker).await.unwrap();
        assert_eq!(r1.enqueued, 0);
        assert_eq!(r1.stabilizing, 1);

        let r2 = tick_folder(&db, &folder, &mut tracker).await.unwrap();
        assert_eq!(r2.enqueued, 1);
        assert_eq!(list_jobs(&db, false, 10).await.unwrap().len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn a_file_that_keeps_growing_is_never_enqueued() {
        let d = temp("growing");
        let (db, fid) = setup(&d, "move").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let mut tracker = StabilityTracker::new();

        for i in 1..4 {
            fs::write(d.join("a.pdf"), vec![b'x'; i * 1000]).unwrap();
            let r = tick_folder(&db, &folder, &mut tracker).await.unwrap();
            assert_eq!(r.enqueued, 0, "a file still being written must not print");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn an_already_queued_file_is_not_enqueued_twice() {
        let d = temp("twice");
        let (db, fid) = setup(&d, "keep").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let mut tracker = StabilityTracker::new();
        fs::write(d.join("a.pdf"), b"payload").unwrap();

        tick_folder(&db, &folder, &mut tracker).await.unwrap();
        tick_folder(&db, &folder, &mut tracker).await.unwrap();
        let r3 = tick_folder(&db, &folder, &mut tracker).await.unwrap();

        assert_eq!(r3.enqueued, 0);
        assert_eq!(list_jobs(&db, false, 10).await.unwrap().len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn files_of_other_types_are_ignored() {
        let d = temp("types");
        let (db, fid) = setup(&d, "move").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let mut tracker = StabilityTracker::new();
        fs::write(d.join("a.txt"), b"payload").unwrap();

        tick_folder(&db, &folder, &mut tracker).await.unwrap();
        let r = tick_folder(&db, &folder, &mut tracker).await.unwrap();
        assert_eq!(r.enqueued, 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn a_missing_folder_reports_path_missing_and_sets_folder_status() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: "/tmp/printy-does-not-exist-xyz".into(),
            poll_interval_secs: 5, file_types: vec!["pdf".into()],
            printer_name: "P".into(), copies: 1, duplex: "simplex".into(),
            color_mode: "mono".into(), post_action: "move".into(),
        }).await.unwrap();
        let mut tracker = StabilityTracker::new();

        let r = tick_folder(&db, &f, &mut tracker).await.unwrap();
        assert!(r.path_missing);
        assert!(r.status_changed, "the first tick reports the transition");
        let back = crate::db::folders::get_folder(&db, f.id).await.unwrap().unwrap();
        assert_eq!(back.status, "path_missing");
    }

    #[tokio::test]
    async fn a_folder_that_stays_missing_reports_the_change_only_once() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: "/tmp/printy-still-does-not-exist-xyz".into(),
            poll_interval_secs: 5, file_types: vec!["pdf".into()],
            printer_name: "P".into(), copies: 1, duplex: "simplex".into(),
            color_mode: "mono".into(), post_action: "move".into(),
        }).await.unwrap();
        let mut tracker = StabilityTracker::new();

        assert!(tick_folder(&db, &f, &mut tracker).await.unwrap().status_changed);
        // The caller re-reads the folder, so the second tick sees the new status.
        let f2 = crate::db::folders::get_folder(&db, f.id).await.unwrap().unwrap();
        let r2 = tick_folder(&db, &f2, &mut tracker).await.unwrap();
        assert!(r2.path_missing);
        assert!(!r2.status_changed, "a folder that stays gone must not re-notify");
    }

    #[tokio::test]
    async fn mark_existing_as_seen_records_files_without_printing_them() {
        let d = temp("seen");
        let (db, fid) = setup(&d, "keep").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        fs::write(d.join("a.pdf"), b"payload").unwrap();
        fs::write(d.join("b.pdf"), b"other").unwrap();

        let n = mark_existing_as_seen(&db, &folder).await.unwrap();
        assert_eq!(n, 2);

        let mut tracker = StabilityTracker::new();
        tick_folder(&db, &folder, &mut tracker).await.unwrap();
        let r = tick_folder(&db, &folder, &mut tracker).await.unwrap();
        assert_eq!(r.enqueued, 0, "pre-existing files must not print by accident");

        let jobs = list_jobs(&db, false, 10).await.unwrap();
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|j| j.state == "done"));
        let _ = fs::remove_dir_all(&d);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test watcher::tick`
Expected: FAIL — `file not found for module 'watcher'`.

- [ ] **Step 3: Write the implementation**

Prepend to `src-tauri/src/watcher/tick.rs`:

```rust
use crate::db::models::WatchFolder;
use crate::db::{folders, jobs, Db};
use crate::intake::dedup::{sha256_file, should_enqueue};
use crate::intake::post::PostAction;
use crate::intake::scan::scan_folder;
use crate::intake::stability::{is_readable, stamp, StabilityTracker};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickReport {
    pub enqueued: usize,
    /// Seen but not yet proven complete.
    pub stabilizing: usize,
    pub path_missing: bool,
    /// True only on the tick where the folder's status actually changed, so a
    /// permanently missing folder notifies once instead of every tick.
    pub status_changed: bool,
}

/// One scan of one folder. Everything time-dependent is derived from the file
/// system, so this is fully testable without sleeping.
pub async fn tick_folder(
    db: &Db,
    folder: &WatchFolder,
    tracker: &mut StabilityTracker,
) -> Result<TickReport, sqlx::Error> {
    let root = Path::new(&folder.path);
    let mut report = TickReport::default();

    let candidates = match scan_folder(root, &folder.types()) {
        Ok(c) => c,
        Err(_) => {
            report.path_missing = true;
            // Write and report only on transition. A folder that stays gone must
            // not raise an alert on every tick — spec section 8, config errors.
            if folder.status != "path_missing" {
                folders::set_folder_status(db, folder.id, "path_missing").await?;
                report.status_changed = true;
            }
            return Ok(report);
        }
    };
    if folder.status != "ok" {
        folders::set_folder_status(db, folder.id, "ok").await?;
        report.status_changed = true;
    }

    let action = PostAction::parse(&folder.post_action);

    for path in candidates {
        let Ok(current) = stamp(&path) else { continue };
        if !tracker.observe(&path, current.clone()) || !is_readable(&path) {
            report.stabilizing += 1;
            continue;
        }

        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        // Cheap stamp check first so we do not hash every file on every tick.
        if jobs::job_seen_for_stamp(
            db, folder.id, &file_name, current.size as i64, current.mtime_ms,
        ).await? {
            continue;
        }

        let Ok(hash) = sha256_file(&path) else { continue };
        if !should_enqueue(db, folder.id, &path, action, &hash).await? {
            continue;
        }

        jobs::enqueue_job(db, &jobs::NewJob {
            folder_id: folder.id,
            file_path: path.to_string_lossy().to_string(),
            file_name,
            size_bytes: current.size as i64,
            mtime_ms: current.mtime_ms,
            sha256: hash,
            printer_name: folder.printer_name.clone(),
            copies: folder.copies,
            duplex: folder.duplex.clone(),
            color_mode: folder.color_mode.clone(),
        }).await?;
        tracker.forget(&path);
        report.enqueued += 1;
    }
    Ok(report)
}

/// Records everything currently in the folder as already handled, without
/// printing it. Used when a folder is added: the opposite default has cost
/// people a paper tray.
pub async fn mark_existing_as_seen(db: &Db, folder: &WatchFolder) -> Result<usize, sqlx::Error> {
    let root = Path::new(&folder.path);
    let Ok(candidates) = scan_folder(root, &folder.types()) else {
        return Ok(0);
    };
    let mut n = 0;
    for path in candidates {
        let Ok(current) = stamp(&path) else { continue };
        let Ok(hash) = sha256_file(&path) else { continue };
        let job = jobs::enqueue_job(db, &jobs::NewJob {
            folder_id: folder.id,
            file_path: path.to_string_lossy().to_string(),
            file_name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            size_bytes: current.size as i64,
            mtime_ms: current.mtime_ms,
            sha256: hash,
            printer_name: folder.printer_name.clone(),
            copies: folder.copies,
            duplex: folder.duplex.clone(),
            color_mode: folder.color_mode.clone(),
        }).await?;
        jobs::mark_done(db, job.id).await?;
        n += 1;
    }
    Ok(n)
}
```

Create `src-tauri/src/watcher/mod.rs`:

```rust
pub mod tick;
```

Add `mod watcher;` to `src-tauri/src/lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test watcher::tick`
Expected: PASS — 7 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/watcher src-tauri/src/lib.rs
git commit -m "feat: add per-folder watcher tick with intake pipeline"
```

---

### Task 16: Runtime wiring — app state, schedulers, printer hold

**Files:**
- Create: `src-tauri/src/watcher/scheduler.rs`, `src-tauri/src/queue/scheduler.rs`
- Modify: `src-tauri/src/watcher/mod.rs`, `src-tauri/src/queue/mod.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `watcher::tick::tick_folder`, `queue::worker::{run_one, QueueOutcome}`, `print::factory::backend`, `db::{jobs::recover_interrupted, settings}`.
- Produces: `AppState`; `watcher::scheduler::spawn_watchers`; `queue::scheduler::spawn_queue_worker`; event topics `printy://job`, `printy://folder`, `printy://queue`.

The queue worker's print call goes through `tokio::task::spawn_blocking` because `Pdfium` is not `Send` and GDI calls block.

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/queue/scheduler.rs` (new file), test module only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_state_reports_when_the_printer_should_be_probed_again() {
        let mut h = PrinterHold::default();
        assert!(h.is_due(0));

        h.engage("Drucker offline", 10_000);
        assert!(!h.is_due(10_000 + PROBE_INTERVAL_MS - 1));
        assert!(h.is_due(10_000 + PROBE_INTERVAL_MS));
        assert_eq!(h.reason(), Some("Drucker offline"));

        h.release();
        assert!(h.is_due(0));
        assert_eq!(h.reason(), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test queue::scheduler`
Expected: FAIL — `cannot find type 'PrinterHold' in this scope`.

- [ ] **Step 3: Write the queue scheduler**

Prepend to `src-tauri/src/queue/scheduler.rs`:

```rust
use crate::db::settings;
use crate::print::factory::backend;
use crate::queue::worker::{run_one, QueueOutcome};
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager};

/// How often a held queue re-probes the printer.
pub const PROBE_INTERVAL_MS: i64 = 60_000;
const TICK_MS: u64 = 500;

/// Runtime-only state. Never persisted — unlike the user's pause switch, a
/// printer hold must clear itself when the printer comes back.
#[derive(Debug, Default)]
pub struct PrinterHold {
    reason: Option<String>,
    next_probe_ms: i64,
}

impl PrinterHold {
    pub fn engage(&mut self, reason: &str, now_ms: i64) {
        self.reason = Some(reason.to_string());
        self.next_probe_ms = now_ms + PROBE_INTERVAL_MS;
    }
    pub fn release(&mut self) {
        self.reason = None;
        self.next_probe_ms = 0;
    }
    pub fn is_due(&self, now_ms: i64) -> bool {
        self.reason.is_none() || now_ms >= self.next_probe_ms
    }
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The single queue worker. Jobs print strictly one after another — printing in
/// parallel would interleave the pages of two documents on one printer.
pub fn spawn_queue_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut hold = PrinterHold::default();
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(TICK_MS));

        loop {
            ticker.tick().await;
            // Clone out of State before any await: the guard is not Send.
            let db = {
                let state = app.state::<AppState>();
                state.db.clone()
            };

            if settings::user_paused(&db).await {
                continue;
            }
            let now = now_ms();
            if !hold.is_due(now) {
                continue;
            }

            let sumatra = settings::sumatra_path(&db).await;
            let db_for_blocking = db.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                let b = backend(sumatra);
                tauri::async_runtime::block_on(run_one(&db_for_blocking, b.as_ref(), now))
            })
            .await;

            let outcome = match outcome {
                Ok(Ok(o)) => o,
                _ => continue,
            };

            match &outcome {
                QueueOutcome::Idle => {}
                QueueOutcome::PrinterHold(reason) => {
                    hold.engage(reason, now);
                    let _ = app.emit("printy://queue", serde_json::json!({
                        "held": true, "reason": reason,
                    }));
                }
                _ => {
                    if hold.reason().is_some() {
                        hold.release();
                        let _ = app.emit("printy://queue",
                            serde_json::json!({ "held": false }));
                    }
                    let _ = app.emit("printy://job", serde_json::json!({
                        "outcome": format!("{outcome:?}"),
                    }));
                }
            }
        }
    });
}
```

Add `pub mod scheduler;` to `src-tauri/src/queue/mod.rs`.

- [ ] **Step 4: Write the watcher scheduler**

Create `src-tauri/src/watcher/scheduler.rs`:

```rust
use crate::db::folders;
use crate::intake::stability::StabilityTracker;
use crate::watcher::tick::tick_folder;
use crate::AppState;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};

const SUPERVISOR_INTERVAL_SECS: u64 = 1;

/// A single supervisor task drives every folder on its own interval. One task
/// per folder would need respawning on every configuration change; this does
/// not, and the work per tick is a directory listing.
pub fn spawn_watchers(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut trackers: HashMap<i64, StabilityTracker> = HashMap::new();
        let mut last_run: HashMap<i64, i64> = HashMap::new();
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(SUPERVISOR_INTERVAL_SECS));

        loop {
            ticker.tick().await;
            let db = {
                let state = app.state::<AppState>();
                state.db.clone()
            };
            if crate::db::settings::user_paused(&db).await {
                continue;
            }

            let now = chrono::Utc::now().timestamp();
            let all = folders::list_folders(&db).await.unwrap_or_default();
            let live: Vec<i64> = all.iter().map(|f| f.id).collect();
            trackers.retain(|id, _| live.contains(id));
            last_run.retain(|id, _| live.contains(id));

            for folder in all.into_iter().filter(|f| f.enabled == 1) {
                let due = last_run
                    .get(&folder.id)
                    .map(|t| now - t >= folder.poll_interval_secs)
                    .unwrap_or(true);
                if !due {
                    continue;
                }
                last_run.insert(folder.id, now);

                let tracker = trackers.entry(folder.id).or_insert_with(StabilityTracker::new);
                if let Ok(report) = tick_folder(&db, &folder, tracker).await {
                    // `status_changed`, not `path_missing`: a folder that stays
                    // gone must notify once, not once per second.
                    if report.enqueued > 0 || report.status_changed {
                        let _ = app.emit("printy://folder", serde_json::json!({
                            "folder_id": folder.id,
                            "enqueued": report.enqueued,
                            "path_missing": report.path_missing,
                        }));
                    }
                }
            }
        }
    });
}
```

Add `pub mod scheduler;` to `src-tauri/src/watcher/mod.rs`.

- [ ] **Step 5: Wire up `lib.rs`**

Replace `src-tauri/src/lib.rs` with:

```rust
mod db;
mod error;
mod intake;
mod print;
mod queue;
mod watcher;

use tauri::Manager;

pub struct AppState {
    pub db: db::Db,
    pub app_data: std::path::PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&dir).ok();
            let db_path = dir.join("printy.sqlite");
            let url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
            let db = tauri::async_runtime::block_on(db::connect(&url))
                .expect("failed to connect/migrate database");

            // A job left mid-print means the app died. Fail it; never reprint.
            let recovered =
                tauri::async_runtime::block_on(db::jobs::recover_interrupted(&db)).unwrap_or(0);
            if recovered > 0 {
                eprintln!("recovered {recovered} interrupted job(s)");
            }

            app.manage(AppState { db, app_data: dir });
            watcher::scheduler::spawn_watchers(app.handle().clone());
            queue::scheduler::spawn_queue_worker(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Run tests and build**

Run: `cd src-tauri && cargo test`
Expected: PASS — all tests from Tasks 1–16.

Run: `cd src-tauri && cargo build`
Expected: build succeeds.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src
git commit -m "feat: wire watcher and queue schedulers into the Tauri runtime"
```

---

### Task 17: Tauri command surface

**Files:**
- Create: `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/folders.rs`, `src-tauri/src/commands/jobs.rs`, `src-tauri/src/commands/printers.rs`, `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/capabilities/default.json`
- Create: `docs/WINDOWS-VERIFICATION.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the commands `list_printers_cmd`, `printer_capabilities_cmd`, `list_folders_cmd`, `create_folder_cmd`, `update_folder_cmd`, `delete_folder_cmd`, `set_folder_enabled_cmd`, `scan_now_cmd`, `list_jobs_cmd`, `reprint_job_cmd`, `get_settings_cmd`, `update_setting_cmd`, `set_global_paused_cmd`, `get_status_cmd`.

- [ ] **Step 1: Write the failing test**

Commands themselves are one-line wrappers and are not unit-tested, matching the house style. The one piece with logic is the status aggregation, so test that. Create `src-tauri/src/commands/jobs.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;
    use crate::db::folders::{create_folder, NewFolder};
    use crate::db::jobs::{enqueue_job, mark_done, mark_printing, NewJob};

    #[tokio::test]
    async fn status_counts_active_folders_and_todays_prints() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: "/tmp/f".into(), poll_interval_secs: 5,
            file_types: vec!["pdf".into()], printer_name: "P".into(), copies: 1,
            duplex: "simplex".into(), color_mode: "mono".into(), post_action: "move".into(),
        }).await.unwrap();

        let j = enqueue_job(&db, &NewJob {
            folder_id: f.id, file_path: "/tmp/f/a.pdf".into(), file_name: "a.pdf".into(),
            size_bytes: 1, mtime_ms: 1, sha256: "h".into(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
        }).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();

        enqueue_job(&db, &NewJob {
            folder_id: f.id, file_path: "/tmp/f/b.pdf".into(), file_name: "b.pdf".into(),
            size_bytes: 1, mtime_ms: 2, sha256: "h2".into(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
        }).await.unwrap();

        let s = build_status(&db).await.unwrap();
        assert_eq!(s.active_folders, 1);
        assert_eq!(s.printed_today, 1);
        assert_eq!(s.waiting, 1);
        assert!(!s.user_paused);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test commands::jobs`
Expected: FAIL — `file not found for module 'commands'`.

- [ ] **Step 3: Write the job and status commands**

Prepend to `src-tauri/src/commands/jobs.rs`:

```rust
use crate::db::models::PrintJob;
use crate::db::{jobs, settings, Db};
use crate::{error::AppResult, AppState};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub active_folders: i64,
    pub printed_today: i64,
    pub waiting: i64,
    pub failed: i64,
    pub user_paused: bool,
}

pub async fn build_status(db: &Db) -> Result<AppStatus, sqlx::Error> {
    let active_folders: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM watch_folder WHERE enabled = 1")
            .fetch_one(db).await?;
    let printed_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM print_job
         WHERE state = 'done' AND date(finished_at) = date('now')",
    ).fetch_one(db).await?;
    let waiting: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM print_job WHERE state IN ('queued', 'retrying', 'printing')",
    ).fetch_one(db).await?;
    let failed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM print_job WHERE state = 'failed'")
            .fetch_one(db).await?;
    Ok(AppStatus {
        active_folders,
        printed_today,
        waiting,
        failed,
        user_paused: settings::user_paused(db).await,
    })
}

#[tauri::command]
pub async fn get_status_cmd(state: State<'_, AppState>) -> AppResult<AppStatus> {
    Ok(build_status(&state.db).await?)
}

#[tauri::command]
pub async fn list_jobs_cmd(
    state: State<'_, AppState>,
    only_failed: bool,
    limit: i64,
) -> AppResult<Vec<PrintJob>> {
    Ok(jobs::list_jobs(&state.db, only_failed, limit.clamp(1, 500)).await?)
}

/// Re-queues a finished job as a fresh one, so history keeps both entries.
#[tauri::command]
pub async fn reprint_job_cmd(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let Some(j) = jobs::get_job(&state.db, id).await? else {
        return Err(crate::error::AppError::Other("Job nicht gefunden".into()));
    };
    if !std::path::Path::new(&j.file_path).exists() {
        return Err(crate::error::AppError::Other(
            "Datei existiert nicht mehr".into(),
        ));
    }
    jobs::enqueue_job(&state.db, &jobs::NewJob {
        folder_id: j.folder_id,
        file_path: j.file_path,
        file_name: j.file_name,
        size_bytes: j.size_bytes,
        mtime_ms: j.mtime_ms,
        sha256: j.sha256,
        printer_name: j.printer_name,
        copies: j.copies,
        duplex: j.duplex,
        color_mode: j.color_mode,
    }).await?;
    Ok(())
}
```

- [ ] **Step 4: Write the remaining command modules**

Create `src-tauri/src/commands/mod.rs`:

```rust
pub mod folders;
pub mod jobs;
pub mod printers;
pub mod settings;
```

Create `src-tauri/src/commands/folders.rs`:

```rust
use crate::db::folders::{self, NewFolder};
use crate::db::models::WatchFolder;
use crate::intake::stability::StabilityTracker;
use crate::watcher::tick::{mark_existing_as_seen, tick_folder};
use crate::{error::AppResult, AppState};
use tauri::State;

#[tauri::command]
pub async fn list_folders_cmd(state: State<'_, AppState>) -> AppResult<Vec<WatchFolder>> {
    Ok(folders::list_folders(&state.db).await?)
}

/// `print_existing = false` marks everything already in the folder as handled.
/// This is the safe default; the opposite must be chosen explicitly.
#[tauri::command]
pub async fn create_folder_cmd(
    state: State<'_, AppState>,
    folder: NewFolder,
    print_existing: bool,
) -> AppResult<WatchFolder> {
    let created = folders::create_folder(&state.db, &folder).await?;
    if !print_existing {
        mark_existing_as_seen(&state.db, &created).await?;
    }
    Ok(created)
}

#[tauri::command]
pub async fn update_folder_cmd(
    state: State<'_, AppState>,
    id: i64,
    folder: NewFolder,
) -> AppResult<WatchFolder> {
    Ok(folders::update_folder(&state.db, id, &folder).await?)
}

#[tauri::command]
pub async fn delete_folder_cmd(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    Ok(folders::delete_folder(&state.db, id).await?)
}

#[tauri::command]
pub async fn set_folder_enabled_cmd(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> AppResult<()> {
    Ok(folders::set_folder_enabled(&state.db, id, enabled).await?)
}

/// Runs two ticks back to back so a settled file is picked up immediately
/// rather than waiting for the next interval.
#[tauri::command]
pub async fn scan_now_cmd(state: State<'_, AppState>, id: i64) -> AppResult<usize> {
    let Some(folder) = folders::get_folder(&state.db, id).await? else {
        return Err(crate::error::AppError::Other("Ordner nicht gefunden".into()));
    };
    let mut tracker = StabilityTracker::new();
    tick_folder(&state.db, &folder, &mut tracker).await?;
    let report = tick_folder(&state.db, &folder, &mut tracker).await?;
    Ok(report.enqueued)
}
```

Create `src-tauri/src/commands/printers.rs`:

```rust
use crate::db::settings;
use crate::print::factory::backend;
use crate::print::{PrinterCapabilities, PrinterInfo};
use crate::{error::AppError, error::AppResult, AppState};
use tauri::State;

#[tauri::command]
pub async fn list_printers_cmd(state: State<'_, AppState>) -> AppResult<Vec<PrinterInfo>> {
    let sumatra = settings::sumatra_path(&state.db).await;
    tokio::task::spawn_blocking(move || backend(sumatra).list_printers())
        .await
        .map_err(|e| AppError::Other(format!("Druckerliste: {e}")))?
        .map_err(|e| AppError::Print(e.message))
}

#[tauri::command]
pub async fn printer_capabilities_cmd(
    state: State<'_, AppState>,
    printer: String,
) -> AppResult<PrinterCapabilities> {
    let sumatra = settings::sumatra_path(&state.db).await;
    tokio::task::spawn_blocking(move || backend(sumatra).capabilities(&printer))
        .await
        .map_err(|e| AppError::Other(format!("Druckerfähigkeiten: {e}")))?
        .map_err(|e| AppError::Print(e.message))
}
```

Create `src-tauri/src/commands/settings.rs`:

```rust
use crate::db::settings;
use crate::{error::AppResult, AppState};
use std::collections::HashMap;
use tauri::State;

const KEYS: &[(&str, &str)] = &[
    ("notification_mode", "all"),
    ("autostart", "0"),
    ("start_minimized", "0"),
    ("theme", "light"),
    ("sumatra_path", ""),
    ("user_paused", "0"),
];

#[tauri::command]
pub async fn get_settings_cmd(state: State<'_, AppState>) -> AppResult<HashMap<String, String>> {
    let mut out = HashMap::new();
    for (key, default) in KEYS {
        let v = settings::get_setting(&state.db, key)
            .await?
            .unwrap_or_else(|| default.to_string());
        out.insert(key.to_string(), v);
    }
    Ok(out)
}

#[tauri::command]
pub async fn update_setting_cmd(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    if !KEYS.iter().any(|(k, _)| *k == key) {
        return Err(crate::error::AppError::Other(format!(
            "Unbekannte Einstellung: {key}"
        )));
    }
    Ok(settings::set_setting(&state.db, &key, &value).await?)
}

#[tauri::command]
pub async fn set_global_paused_cmd(state: State<'_, AppState>, paused: bool) -> AppResult<()> {
    Ok(settings::set_setting(&state.db, "user_paused", if paused { "1" } else { "0" }).await?)
}
```

- [ ] **Step 5: Register the commands**

Add `mod commands;` to `src-tauri/src/lib.rs` and insert before `.run(...)`:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::folders::list_folders_cmd,
            commands::folders::create_folder_cmd,
            commands::folders::update_folder_cmd,
            commands::folders::delete_folder_cmd,
            commands::folders::set_folder_enabled_cmd,
            commands::folders::scan_now_cmd,
            commands::jobs::get_status_cmd,
            commands::jobs::list_jobs_cmd,
            commands::jobs::reprint_job_cmd,
            commands::printers::list_printers_cmd,
            commands::printers::printer_capabilities_cmd,
            commands::settings::get_settings_cmd,
            commands::settings::update_setting_cmd,
            commands::settings::set_global_paused_cmd,
        ])
```

Spec section 11 also lists `pick_folder` and `open_path`. Neither needs a Rust
command: the folder picker is `@tauri-apps/plugin-dialog`'s `open({ directory:
true })` and revealing a folder is `@tauri-apps/plugin-opener`'s `revealItemInDir`,
both called straight from the frontend. That is why the capability below grants
`dialog:allow-open` — without it those calls are silently denied at runtime.

Replace `src-tauri/capabilities/default.json` with:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "notification:default",
    "dialog:allow-open"
  ]
}
```

- [ ] **Step 6: Write the Windows verification checklist**

Create `docs/WINDOWS-VERIFICATION.md`:

```markdown
# Windows verification checklist

The GDI printing path cannot be unit-tested — Win32 handles against a real
spooler cannot be mocked honestly. Run this list once on a Windows machine
before any release.

1. The printer list matches the Windows printer list, and the system default is
   preselected.
2. A single-page PDF prints, correctly oriented and scaled inside the margins.
3. A multi-page PDF prints in the correct page order.
4. Two copies produce two copies.
5. Duplex produces a double-sided document.
6. Mono mode on a colour printer produces greyscale output.
7. Duplex and colour controls are reported unsupported for a printer that lacks
   them (`printer_capabilities_cmd`).
8. A JPG and a PNG print through the same path.
9. Switching the printer off mid-queue holds the queue and consumes no attempt;
   switching it back on resumes within a minute with no lost jobs.
10. A corrupt PDF fails on its own after three attempts, lands in `failed/`, and
    the queue continues with the next job.
11. Killing the app mid-print leaves that job as `failed` with kind
    `interrupted` on the next start — it must never reprint by itself.
12. With SumatraPDF installed and configured, a PDF that pdfium refuses still
    prints; with it absent, the job fails with a clear message.
```

- [ ] **Step 7: Run tests and build**

Run: `cd src-tauri && cargo test`
Expected: PASS — all tests.

Run: `cd src-tauri && cargo check --target x86_64-pc-windows-msvc`
Expected: PASS.

Run: `npm run tauri dev`
Expected: the app window opens; the default Vite page renders. The backend
schedulers are running — no printing happens yet because no folder is
configured. The UI is built in the follow-up plan.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: expose folder, job, printer and settings commands to the frontend"
```

---

## What this plan does not cover

Deliberately deferred to the follow-up UI plan: branding and the Printy logo,
`theme.css`, the sidebar shell, the Ordner / Verlauf / Einstellungen / Über
screens, the folder dialog with capability-driven control disabling, the tray
icon and its state colours, close-to-tray, autostart wiring, desktop
notifications, and the frontend test suite.
