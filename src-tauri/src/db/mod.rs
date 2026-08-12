pub mod models;
pub mod folders;
pub mod jobs;

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
