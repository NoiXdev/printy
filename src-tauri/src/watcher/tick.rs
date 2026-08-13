use crate::db::models::WatchFolder;
use crate::db::{folders, jobs, Db};
use crate::intake::dedup::{sha256_file, should_enqueue};
use crate::intake::post::PostAction;
use crate::intake::scan::scan_folder;
use crate::intake::stability::{is_readable, stamp, StabilityTracker};
use std::path::Path;
use std::time::Duration;

/// The real-world delay `scan_now` waits out between its two observations.
/// Anything shorter defeats the stability check: two stamps taken
/// microseconds apart are trivially identical even for a file still being
/// written.
pub const SCAN_NOW_STABILITY_DELAY: Duration = Duration::from_secs(1);

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
            fit_to_page: folder.fit_to_page != 0,
        }).await?;
        tracker.forget(&path);
        report.enqueued += 1;
    }
    Ok(report)
}

/// An on-demand scan of one folder: two observations separated by `delay` on
/// a fresh tracker, so a file that is still being written genuinely fails the
/// stability check instead of being compared against itself microseconds
/// later. Callers pass `SCAN_NOW_STABILITY_DELAY`; tests pass something much
/// shorter so they do not have to sleep for real.
pub async fn scan_now(
    db: &Db,
    folder: &WatchFolder,
    delay: Duration,
) -> Result<TickReport, sqlx::Error> {
    let mut tracker = StabilityTracker::new();
    tick_folder(db, folder, &mut tracker).await?;
    tokio::time::sleep(delay).await;
    tick_folder(db, folder, &mut tracker).await
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
            fit_to_page: folder.fit_to_page != 0,
        }).await?;
        jobs::mark_done(db, job.id).await?;
        n += 1;
    }
    Ok(n)
}

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
        setup_with_fit(dir, action, true).await
    }

    async fn setup_with_fit(
        dir: &std::path::Path, action: &str, fit_to_page: bool,
    ) -> (crate::db::Db, i64) {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &NewFolder {
            name: "F".into(), path: dir.to_string_lossy().into(), poll_interval_secs: 5,
            file_types: vec!["pdf".into()], printer_name: "P".into(), copies: 1,
            duplex: "simplex".into(), color_mode: "mono".into(), post_action: action.into(),
            fit_to_page,
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

    /// Pre-seeds the tracker with the file's current stamp so this call's own
    /// `tracker.observe` inside `tick_folder` immediately reports the file
    /// stable, instead of the ordinary two-tick stability gate. That gate
    /// resets on every successful enqueue (`tracker.forget`), so without this
    /// a tick right after an enqueue would report `enqueued == 0` merely
    /// because the file looks "new" again to the tracker -- never actually
    /// reaching the `job_seen_for_stamp` dedup check this suite exists to pin.
    async fn seeded_tick(
        db: &crate::db::Db,
        folder: &WatchFolder,
        tracker: &mut StabilityTracker,
        path: &std::path::Path,
    ) -> TickReport {
        let s = stamp(path).unwrap();
        tracker.observe(path, s);
        tick_folder(db, folder, tracker).await.unwrap()
    }

    /// At a 1-second poll interval, a job sitting in the queue while its file
    /// is scanned again and again is the common case, not a rare race. A file
    /// whose already-enqueued job is still `queued` must not be enqueued a
    /// second time. This fails if `job_seen_for_stamp` (or the check against
    /// it in `tick_folder`) is removed: the file would be reported as
    /// stable-and-unseen on every seeded tick and get enqueued again.
    #[tokio::test]
    async fn a_file_whose_job_is_queued_is_not_enqueued_again() {
        let d = temp("dup_queued");
        let (db, fid) = setup(&d, "keep").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let path = d.join("a.pdf");
        fs::write(&path, b"payload").unwrap();
        let mut tracker = StabilityTracker::new();

        let r1 = seeded_tick(&db, &folder, &mut tracker, &path).await;
        assert_eq!(r1.enqueued, 1);
        let jobs = list_jobs(&db, false, 10).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].state, "queued");

        let r2 = seeded_tick(&db, &folder, &mut tracker, &path).await;
        assert_eq!(
            r2.enqueued, 0,
            "a file whose job is already queued must not be enqueued again"
        );
        assert_eq!(list_jobs(&db, false, 10).await.unwrap().len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    /// The file stays on disk for as long as a job for it is `printing` -- the
    /// post-print action (move/delete) only runs once the print itself
    /// finishes. At a 1-second interval, a scan landing squarely inside that
    /// window is likely, not theoretical, so it must not enqueue a second job
    /// for the same file.
    #[tokio::test]
    async fn a_file_whose_job_is_printing_is_not_enqueued_again() {
        let d = temp("dup_printing");
        let (db, fid) = setup(&d, "keep").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let path = d.join("a.pdf");
        fs::write(&path, b"payload").unwrap();
        let mut tracker = StabilityTracker::new();

        let r1 = seeded_tick(&db, &folder, &mut tracker, &path).await;
        assert_eq!(r1.enqueued, 1);
        let jobs = list_jobs(&db, false, 10).await.unwrap();
        crate::db::jobs::mark_printing(&db, jobs[0].id).await.unwrap();

        let r2 = seeded_tick(&db, &folder, &mut tracker, &path).await;
        assert_eq!(
            r2.enqueued, 0,
            "a file whose job is printing must not be enqueued again"
        );
        let after = list_jobs(&db, false, 10).await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].state, "printing");
        let _ = fs::remove_dir_all(&d);
    }

    /// A job that failed once and is waiting out its retry backoff is still a
    /// job for this exact file -- re-enqueuing it would print it twice once
    /// both the retry and the duplicate eventually fire.
    #[tokio::test]
    async fn a_file_whose_job_is_retrying_is_not_enqueued_again() {
        let d = temp("dup_retrying");
        let (db, fid) = setup(&d, "keep").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let path = d.join("a.pdf");
        fs::write(&path, b"payload").unwrap();
        let mut tracker = StabilityTracker::new();

        let r1 = seeded_tick(&db, &folder, &mut tracker, &path).await;
        assert_eq!(r1.enqueued, 1);
        let jobs = list_jobs(&db, false, 10).await.unwrap();
        crate::db::jobs::mark_printing(&db, jobs[0].id).await.unwrap();
        crate::db::jobs::mark_retrying(&db, jobs[0].id, "file", "kaputt", i64::MAX)
            .await
            .unwrap();

        let r2 = seeded_tick(&db, &folder, &mut tracker, &path).await;
        assert_eq!(
            r2.enqueued, 0,
            "a file whose job is retrying must not be enqueued again"
        );
        let after = list_jobs(&db, false, 10).await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].state, "retrying");
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
            color_mode: "mono".into(), post_action: "move".into(), fit_to_page: true,
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
            color_mode: "mono".into(), post_action: "move".into(), fit_to_page: true,
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

    #[tokio::test]
    async fn tick_folder_copies_fit_to_page_false_onto_the_enqueued_job() {
        let d = temp("fitoff");
        let (db, fid) = setup_with_fit(&d, "move", false).await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let mut tracker = StabilityTracker::new();
        fs::write(d.join("a.pdf"), b"payload").unwrap();

        tick_folder(&db, &folder, &mut tracker).await.unwrap();
        let r2 = tick_folder(&db, &folder, &mut tracker).await.unwrap();
        assert_eq!(r2.enqueued, 1);

        let jobs = list_jobs(&db, false, 10).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].fit_to_page, 0,
            "a folder with fit_to_page = false must produce a job with fit_to_page = 0"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn mark_existing_as_seen_also_copies_fit_to_page_from_the_folder() {
        let d = temp("seenfit");
        let (db, fid) = setup_with_fit(&d, "keep", false).await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        fs::write(d.join("a.pdf"), b"payload").unwrap();

        mark_existing_as_seen(&db, &folder).await.unwrap();

        let jobs = list_jobs(&db, false, 10).await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].fit_to_page, 0);
        let _ = fs::remove_dir_all(&d);
    }

    /// A file that grows in the window between `scan_now`'s two observations
    /// must not be enqueued. A background task performs the growth after a
    /// short sleep so this proves real separation in time rather than two
    /// stamps compared microseconds apart -- while still running in
    /// milliseconds instead of the production ~1s delay.
    #[tokio::test]
    async fn scan_now_rejects_a_file_that_grows_between_observations() {
        let d = temp("scan_now_growing");
        let (db, fid) = setup(&d, "move").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        let path = d.join("a.pdf");
        fs::write(&path, b"short").unwrap();

        let growth_path = path.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            fs::write(&growth_path, b"this file grew a lot between observations").unwrap();
        });

        let report = scan_now(&db, &folder, Duration::from_millis(50)).await.unwrap();
        assert_eq!(
            report.enqueued, 0,
            "a file that grew between the two observations must not be enqueued"
        );
        assert_eq!(list_jobs(&db, false, 10).await.unwrap().len(), 0);
        let _ = fs::remove_dir_all(&d);
    }

    /// The mirror case: a file that does not change between the two
    /// observations must be enqueued, proving `scan_now` still does its job
    /// once real separation in time is introduced.
    #[tokio::test]
    async fn scan_now_enqueues_a_file_that_stays_stable() {
        let d = temp("scan_now_stable");
        let (db, fid) = setup(&d, "move").await;
        let folder = crate::db::folders::get_folder(&db, fid).await.unwrap().unwrap();
        fs::write(d.join("a.pdf"), b"payload").unwrap();

        let report = scan_now(&db, &folder, Duration::from_millis(10)).await.unwrap();
        assert_eq!(report.enqueued, 1);
        assert_eq!(list_jobs(&db, false, 10).await.unwrap().len(), 1);
        let _ = fs::remove_dir_all(&d);
    }
}
