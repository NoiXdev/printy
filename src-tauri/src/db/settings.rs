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

pub async fn pdfium_path(db: &Db) -> Option<String> {
    get_setting(db, "pdfium_path").await.ok().flatten().filter(|s| !s.is_empty())
}

/// New folders prefill their per-folder poll interval from this; existing
/// folders are never touched by it. Floored at 1, matching the per-folder
/// `MIN_POLL_INTERVAL_SECS`.
pub async fn default_poll_interval_secs(db: &Db) -> i64 {
    get_setting(db, "default_poll_interval_secs")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1)
}

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
        assert!(pdfium_path(&db).await.is_none());
        assert_eq!(default_poll_interval_secs(&db).await, 1);
    }

    #[tokio::test]
    async fn pdfium_path_treats_an_empty_string_as_not_configured() {
        let db = connect("sqlite::memory:").await.unwrap();
        set_setting(&db, "pdfium_path", "").await.unwrap();
        assert!(pdfium_path(&db).await.is_none());
        set_setting(&db, "pdfium_path", "/opt/pdfium/libpdfium.dylib").await.unwrap();
        assert_eq!(
            pdfium_path(&db).await.as_deref(),
            Some("/opt/pdfium/libpdfium.dylib"),
        );
    }

    #[tokio::test]
    async fn default_poll_interval_secs_is_floored_and_ignores_garbage() {
        let db = connect("sqlite::memory:").await.unwrap();
        set_setting(&db, "default_poll_interval_secs", "0").await.unwrap();
        assert_eq!(default_poll_interval_secs(&db).await, 1);
        set_setting(&db, "default_poll_interval_secs", "banana").await.unwrap();
        assert_eq!(default_poll_interval_secs(&db).await, 1);
        set_setting(&db, "default_poll_interval_secs", "7").await.unwrap();
        assert_eq!(default_poll_interval_secs(&db).await, 7);
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
