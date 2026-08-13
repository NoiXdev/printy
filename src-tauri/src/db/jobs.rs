use crate::db::models::{FolderDeleteImpact, PrintJob};
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
    pub fit_to_page: bool,
}

pub async fn enqueue_job(db: &Db, n: &NewJob) -> Result<PrintJob, sqlx::Error> {
    sqlx::query_as::<_, PrintJob>(
        "INSERT INTO print_job
           (folder_id, file_path, file_name, size_bytes, mtime_ms, sha256,
            state, printer_name, copies, duplex, color_mode, fit_to_page)
         VALUES (?, ?, ?, ?, ?, ?, 'queued', ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(n.folder_id).bind(&n.file_path).bind(&n.file_name).bind(n.size_bytes)
    .bind(n.mtime_ms).bind(&n.sha256).bind(&n.printer_name).bind(n.copies)
    .bind(&n.duplex).bind(&n.color_mode).bind(n.fit_to_page as i64)
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

/// Applied when a folder is edited (spec section "Amended again (2026-08-13):
/// editing a folder updates its waiting jobs"). Rewrites the printer/copies/
/// duplex/colour/fit-to-page snapshot on jobs that have not yet been handed to
/// the spooler, so a corrected printer actually takes effect on files already
/// sitting in the queue instead of going to the wrong device anyway.
///
/// Deliberately excludes:
/// - `printing`: already handed to the spooler; changing it would either do
///   nothing or switch device mid-document.
/// - `done` and `failed`: history. Rewriting what a job *was* printed with
///   would make the record lie about what actually came out of the printer.
pub async fn update_waiting_jobs_for_folder(
    db: &Db,
    folder_id: i64,
    printer_name: &str,
    copies: i64,
    duplex: &str,
    color_mode: &str,
    fit_to_page: bool,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE print_job SET printer_name = ?, copies = ?, duplex = ?,
           color_mode = ?, fit_to_page = ?
         WHERE folder_id = ? AND state IN ('queued', 'retrying')",
    )
    .bind(printer_name).bind(copies).bind(duplex).bind(color_mode)
    .bind(fit_to_page as i64).bind(folder_id)
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

/// Splits one folder's jobs into "still waiting" and "history", so a folder
/// delete confirmation can name concretely what a cascade delete takes with
/// it instead of just warning in the abstract. Deliberately two scalar
/// queries rather than one grouped query: each half maps to a fixed, known
/// set of states, and that is clearer to read (and to keep in sync with
/// `JobState`) than reducing a `GROUP BY state` result set back down to two
/// buckets.
pub async fn count_delete_impact(
    db: &Db,
    folder_id: i64,
) -> Result<FolderDeleteImpact, sqlx::Error> {
    let waiting: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM print_job
         WHERE folder_id = ? AND state IN ('queued', 'retrying', 'printing')",
    )
    .bind(folder_id)
    .fetch_one(db)
    .await?;

    let history: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM print_job
         WHERE folder_id = ? AND state IN ('done', 'failed')",
    )
    .bind(folder_id)
    .fetch_one(db)
    .await?;

    Ok(FolderDeleteImpact { waiting, history })
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
            fit_to_page: true,
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
            fit_to_page: true,
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

    #[tokio::test]
    async fn mark_done_clears_stale_error_fields() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        // Set error fields via mark_retrying
        mark_retrying(&db, j.id, "file", "kaputt", 10_000).await.unwrap();
        let with_errors = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(with_errors.error_kind.as_deref(), Some("file"));
        assert_eq!(with_errors.error_message.as_deref(), Some("kaputt"));
        assert_eq!(with_errors.next_attempt_at, Some(10_000));
        // Resume printing and mark done
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();
        let cleaned = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(cleaned.state, JobState::Done.as_str());
        assert_eq!(cleaned.error_kind.as_deref(), None);
        assert_eq!(cleaned.error_message.as_deref(), None);
        assert_eq!(cleaned.next_attempt_at, None);
        assert!(cleaned.finished_at.is_some());
    }

    #[tokio::test]
    async fn mark_printing_sets_started_at() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        let queued = get_job(&db, j.id).await.unwrap().unwrap();
        assert!(queued.started_at.is_none());
        mark_printing(&db, j.id).await.unwrap();
        let printing = get_job(&db, j.id).await.unwrap().unwrap();
        assert!(printing.started_at.is_some());
    }

    #[tokio::test]
    async fn requeue_clears_a_previously_set_next_attempt_at() {
        let (db, fid) = setup().await;
        let j = enqueue_job(&db, &job(fid, "a.pdf", "h1")).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        // Set a far-future next_attempt_at via mark_retrying
        mark_retrying(&db, j.id, "file", "kaputt", 1_000_000).await.unwrap();
        let retrying = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(retrying.attempts, 1);
        assert_eq!(retrying.next_attempt_at, Some(1_000_000));
        // Resume printing and requeue
        mark_printing(&db, j.id).await.unwrap();
        requeue_job(&db, j.id).await.unwrap();
        let requeued = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(requeued.state, JobState::Queued.as_str());
        assert_eq!(requeued.attempts, 1);  // attempt still consumed
        assert_eq!(requeued.next_attempt_at, None);  // but backoff cleared
        assert_eq!(requeued.started_at, None);
        // Job must be visible to next_due_job at a time well before the far-future timestamp
        let due = next_due_job(&db, 0).await.unwrap().unwrap();
        assert_eq!(due.id, j.id);
        assert_eq!(due.file_name, "a.pdf");
    }

    #[tokio::test]
    async fn recover_interrupted_touches_only_printing_jobs() {
        let (db, fid) = setup().await;
        // Create four jobs in different states
        let queued = enqueue_job(&db, &job(fid, "q.pdf", "h_queued")).await.unwrap();
        let printing_one = enqueue_job(&db, &job(fid, "p.pdf", "h_printing")).await.unwrap();
        mark_printing(&db, printing_one.id).await.unwrap();
        let retrying_one = enqueue_job(&db, &job(fid, "r.pdf", "h_retrying")).await.unwrap();
        mark_printing(&db, retrying_one.id).await.unwrap();
        mark_retrying(&db, retrying_one.id, "file", "kaputt", 10_000).await.unwrap();
        let done_one = enqueue_job(&db, &job(fid, "d.pdf", "h_done")).await.unwrap();
        mark_printing(&db, done_one.id).await.unwrap();
        mark_done(&db, done_one.id).await.unwrap();

        // Call recover_interrupted
        let affected = recover_interrupted(&db).await.unwrap();
        assert_eq!(affected, 1);  // Only the printing job should be affected

        // Verify the printing job is now failed with the right error
        let recovered = get_job(&db, printing_one.id).await.unwrap().unwrap();
        assert_eq!(recovered.state, JobState::Failed.as_str());
        assert_eq!(recovered.error_kind.as_deref(), Some("interrupted"));

        // Verify the other three are unchanged
        let still_queued = get_job(&db, queued.id).await.unwrap().unwrap();
        assert_eq!(still_queued.state, JobState::Queued.as_str());
        let still_retrying = get_job(&db, retrying_one.id).await.unwrap().unwrap();
        assert_eq!(still_retrying.state, JobState::Retrying.as_str());
        let still_done = get_job(&db, done_one.id).await.unwrap().unwrap();
        assert_eq!(still_done.state, JobState::Done.as_str());
    }

    #[tokio::test]
    async fn count_delete_impact_splits_waiting_from_history() {
        let (db, fid) = setup().await;

        // One job in each of the five states, all present at once.
        enqueue_job(&db, &job(fid, "queued.pdf", "h1")).await.unwrap();

        let printing = enqueue_job(&db, &job(fid, "printing.pdf", "h2")).await.unwrap();
        mark_printing(&db, printing.id).await.unwrap();

        let retrying = enqueue_job(&db, &job(fid, "retrying.pdf", "h3")).await.unwrap();
        mark_printing(&db, retrying.id).await.unwrap();
        mark_retrying(&db, retrying.id, "file", "kaputt", 10_000).await.unwrap();

        let done = enqueue_job(&db, &job(fid, "done.pdf", "h4")).await.unwrap();
        mark_printing(&db, done.id).await.unwrap();
        mark_done(&db, done.id).await.unwrap();

        let failed = enqueue_job(&db, &job(fid, "failed.pdf", "h5")).await.unwrap();
        mark_failed(&db, failed.id, "file", "kaputt").await.unwrap();

        let impact = count_delete_impact(&db, fid).await.unwrap();
        // queued + printing + retrying = 3; done + failed = 2.
        assert_eq!(impact.waiting, 3);
        assert_eq!(impact.history, 2);
    }

    #[tokio::test]
    async fn count_delete_impact_is_zero_for_a_folder_without_jobs() {
        let (db, fid) = setup().await;
        let impact = count_delete_impact(&db, fid).await.unwrap();
        assert_eq!(impact.waiting, 0);
        assert_eq!(impact.history, 0);
    }

    #[tokio::test]
    async fn count_delete_impact_only_counts_the_given_folder() {
        let (db, fid) = setup().await;
        let other = create_folder(
            &db,
            &NewFolder {
                name: "Other".into(),
                path: "/tmp/other".into(),
                poll_interval_secs: 5,
                file_types: vec!["pdf".into()],
                printer_name: "P".into(),
                copies: 1,
                duplex: "simplex".into(),
                color_mode: "mono".into(),
                post_action: "move".into(),
                fit_to_page: true,
            },
        )
        .await
        .unwrap();

        enqueue_job(&db, &job(fid, "mine.pdf", "h1")).await.unwrap();
        enqueue_job(&db, &job(other.id, "theirs.pdf", "h2")).await.unwrap();

        let impact = count_delete_impact(&db, fid).await.unwrap();
        assert_eq!(impact.waiting, 1);
        assert_eq!(impact.history, 0);
    }

    #[tokio::test]
    async fn enqueue_job_snapshots_fit_to_page_onto_the_job() {
        let (db, fid) = setup().await;

        let mut off = job(fid, "a.pdf", "h1");
        off.fit_to_page = false;
        let j = enqueue_job(&db, &off).await.unwrap();
        assert_eq!(j.fit_to_page, 0);

        // A second job enqueued with the opposite flag proves the value is
        // carried per-call from the `NewJob` the caller builds, not read back
        // from live folder state — this is the snapshot semantics the spec
        // requires: changing a folder's fit_to_page must never alter jobs
        // already queued (the folder -> job copy itself happens in the
        // not-yet-built queue/intake layer; this test covers the db layer's
        // half of that contract, which is what `enqueue_job` owns).
        let mut on = job(fid, "b.pdf", "h2");
        on.fit_to_page = true;
        let j2 = enqueue_job(&db, &on).await.unwrap();
        assert_eq!(j2.fit_to_page, 1);

        // The first job's row is untouched by the second enqueue.
        let refetched = get_job(&db, j.id).await.unwrap().unwrap();
        assert_eq!(refetched.fit_to_page, 0);
    }
}
