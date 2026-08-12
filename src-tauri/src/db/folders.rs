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
