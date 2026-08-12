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
        fit_to_page: j.fit_to_page != 0,
    }).await?;
    Ok(())
}

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
            fit_to_page: true,
        }).await.unwrap();

        let j = enqueue_job(&db, &NewJob {
            folder_id: f.id, file_path: "/tmp/f/a.pdf".into(), file_name: "a.pdf".into(),
            size_bytes: 1, mtime_ms: 1, sha256: "h".into(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
            fit_to_page: true,
        }).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();

        enqueue_job(&db, &NewJob {
            folder_id: f.id, file_path: "/tmp/f/b.pdf".into(), file_name: "b.pdf".into(),
            size_bytes: 1, mtime_ms: 2, sha256: "h2".into(), printer_name: "P".into(),
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(),
            fit_to_page: true,
        }).await.unwrap();

        let s = build_status(&db).await.unwrap();
        assert_eq!(s.active_folders, 1);
        assert_eq!(s.printed_today, 1);
        assert_eq!(s.waiting, 1);
        assert!(!s.user_paused);
    }
}
