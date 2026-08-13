use crate::db::jobs;
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
    pub fit_to_page: bool,
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
            copies, duplex, color_mode, post_action, fit_to_page)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(&n.name).bind(&n.path).bind(interval).bind(types).bind(&n.printer_name)
    .bind(copies).bind(&n.duplex).bind(&n.color_mode).bind(&n.post_action)
    .bind(n.fit_to_page as i64)
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
    let updated = sqlx::query_as::<_, WatchFolder>(
        "UPDATE watch_folder SET
           name = ?, path = ?, poll_interval_secs = ?, file_types = ?,
           printer_name = ?, copies = ?, duplex = ?, color_mode = ?,
           post_action = ?, fit_to_page = ?, updated_at = datetime('now')
         WHERE id = ? RETURNING *",
    )
    .bind(&n.name).bind(&n.path).bind(interval).bind(types).bind(&n.printer_name)
    .bind(copies).bind(&n.duplex).bind(&n.color_mode).bind(&n.post_action)
    .bind(n.fit_to_page as i64).bind(id)
    .fetch_one(db)
    .await?;

    // Editing a folder rewrites its still-waiting jobs -- see
    // `jobs::update_waiting_jobs_for_folder` for exactly which states and why.
    // Sourced from the row just persisted (not `n`) so the snapshot matches
    // the clamped values actually stored on the folder.
    jobs::update_waiting_jobs_for_folder(
        db,
        id,
        &updated.printer_name,
        updated.copies,
        &updated.duplex,
        &updated.color_mode,
        updated.fit_to_page != 0,
    )
    .await?;

    Ok(updated)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;
    use crate::db::jobs::{
        enqueue_job, get_job, mark_done, mark_failed, mark_printing, mark_retrying, NewJob,
    };

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
            fit_to_page: true,
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

    #[tokio::test]
    async fn update_folder_changes_every_field() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &sample()).await.unwrap();
        let original_id = f.id;

        let updated = NewFolder {
            name: "Invoices".into(),
            path: "/tmp/printy-invoices".into(),
            poll_interval_secs: 10,
            file_types: vec!["tiff".into(), "jpg".into()],
            printer_name: "Xerox".into(),
            copies: 3,
            duplex: "duplex".into(),
            color_mode: "color".into(),
            post_action: "delete".into(),
            fit_to_page: false,
        };

        let f = update_folder(&db, f.id, &updated).await.unwrap();

        assert_eq!(f.id, original_id);
        assert_eq!(f.name, "Invoices");
        assert_eq!(f.path, "/tmp/printy-invoices");
        assert_eq!(f.poll_interval_secs, 10);
        assert_eq!(f.types(), vec!["tiff".to_string(), "jpg".to_string()]);
        assert_eq!(f.printer_name, "Xerox");
        assert_eq!(f.copies, 3);
        assert_eq!(f.duplex, "duplex");
        assert_eq!(f.color_mode, "color");
        assert_eq!(f.post_action, "delete");
        assert_eq!(f.fit_to_page, 0);
        assert!(!f.updated_at.is_empty());
    }

    #[tokio::test]
    async fn fit_to_page_defaults_to_true_and_persists_through_an_update() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &sample()).await.unwrap();
        assert_eq!(f.fit_to_page, 1);

        let mut off = sample();
        off.fit_to_page = false;
        let updated = update_folder(&db, f.id, &off).await.unwrap();
        assert_eq!(updated.fit_to_page, 0);

        let refetched = get_folder(&db, f.id).await.unwrap().unwrap();
        assert_eq!(refetched.fit_to_page, 0);
    }

    #[tokio::test]
    async fn update_folder_rewrites_only_queued_and_retrying_jobs() {
        let db = connect("sqlite::memory:").await.unwrap();
        let f = create_folder(&db, &sample()).await.unwrap();

        let template = |name: &str, hash: &str| NewJob {
            folder_id: f.id,
            file_path: format!("/tmp/printy-scans/{name}"),
            file_name: name.into(),
            size_bytes: 10,
            mtime_ms: 1,
            sha256: hash.into(),
            printer_name: "Brother".into(),
            copies: 1,
            duplex: "simplex".into(),
            color_mode: "mono".into(),
            fit_to_page: true,
        };

        // One job in each of the five states, all present at once.
        let queued = enqueue_job(&db, &template("queued.pdf", "h1")).await.unwrap();

        let printing = enqueue_job(&db, &template("printing.pdf", "h2")).await.unwrap();
        mark_printing(&db, printing.id).await.unwrap();

        let retrying = enqueue_job(&db, &template("retrying.pdf", "h3")).await.unwrap();
        mark_printing(&db, retrying.id).await.unwrap();
        mark_retrying(&db, retrying.id, "file", "kaputt", 10_000).await.unwrap();

        let done = enqueue_job(&db, &template("done.pdf", "h4")).await.unwrap();
        mark_printing(&db, done.id).await.unwrap();
        mark_done(&db, done.id).await.unwrap();

        let failed = enqueue_job(&db, &template("failed.pdf", "h5")).await.unwrap();
        mark_failed(&db, failed.id, "file", "kaputt").await.unwrap();

        // Edit the folder: new printer, copies, duplex, colour and fit_to_page.
        let mut edited = sample();
        edited.printer_name = "Xerox".into();
        edited.copies = 5;
        edited.duplex = "long_edge".into();
        edited.color_mode = "color".into();
        edited.fit_to_page = false;
        update_folder(&db, f.id, &edited).await.unwrap();

        // The two waiting jobs picked up the new settings.
        for id in [queued.id, retrying.id] {
            let j = get_job(&db, id).await.unwrap().unwrap();
            assert_eq!(j.printer_name, "Xerox");
            assert_eq!(j.copies, 5);
            assert_eq!(j.duplex, "long_edge");
            assert_eq!(j.color_mode, "color");
            assert_eq!(j.fit_to_page, 0);
        }

        // The printing job is never touched -- already handed to the spooler.
        let printing_after = get_job(&db, printing.id).await.unwrap().unwrap();
        assert_eq!(printing_after.printer_name, "Brother");
        assert_eq!(printing_after.copies, 1);
        assert_eq!(printing_after.duplex, "simplex");
        assert_eq!(printing_after.color_mode, "mono");
        assert_eq!(printing_after.fit_to_page, 1);

        // done and failed jobs are history and must not be rewritten either.
        let done_after = get_job(&db, done.id).await.unwrap().unwrap();
        assert_eq!(done_after.printer_name, "Brother");
        assert_eq!(done_after.copies, 1);
        assert_eq!(done_after.duplex, "simplex");
        assert_eq!(done_after.color_mode, "mono");
        assert_eq!(done_after.fit_to_page, 1);

        let failed_after = get_job(&db, failed.id).await.unwrap().unwrap();
        assert_eq!(failed_after.printer_name, "Brother");
        assert_eq!(failed_after.copies, 1);
        assert_eq!(failed_after.duplex, "simplex");
        assert_eq!(failed_after.color_mode, "mono");
        assert_eq!(failed_after.fit_to_page, 1);
    }
}
