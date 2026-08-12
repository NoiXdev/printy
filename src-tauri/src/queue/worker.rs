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
        // Snapshotted onto the job at enqueue time (watcher::tick); forwarded
        // here unchanged so a later folder edit never touches a queued job.
        fit_to_page: job.fit_to_page != 0,
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
            fit_to_page: true,
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
            fit_to_page: false,
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
        // job.fit_to_page must be forwarded onto the PrintRequest unchanged --
        // this is the job-side half of the fit_to_page link; the folder-side
        // half (folder -> NewJob) is covered in watcher::tick's tests.
        assert!(!jobs[0].fit_to_page);
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
