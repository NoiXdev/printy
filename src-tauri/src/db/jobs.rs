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
