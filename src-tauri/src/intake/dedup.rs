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
            fit_to_page: true,
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
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(), fit_to_page: true,
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
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(), fit_to_page: true,
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
            copies: 1, duplex: "simplex".into(), color_mode: "mono".into(), fit_to_page: true,
        }).await.unwrap();
        mark_printing(&db, j.id).await.unwrap();
        mark_done(&db, j.id).await.unwrap();

        // The file left the folder in move mode, so a file sitting there again
        // is genuinely new work and must print.
        assert!(should_enqueue(&db, fid, &f, PostAction::Move, &hash).await.unwrap());
        let _ = fs::remove_dir_all(&d);
    }
}
