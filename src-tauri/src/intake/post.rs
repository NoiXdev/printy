use crate::intake::scan::{FAILED_DIR, PRINTED_DIR};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAction {
    Move,
    Keep,
    Delete,
}

impl PostAction {
    pub fn parse(s: &str) -> Self {
        match s {
            "keep" => PostAction::Keep,
            "delete" => PostAction::Delete,
            _ => PostAction::Move,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PostAction::Move => "move",
            PostAction::Keep => "keep",
            PostAction::Delete => "delete",
        }
    }
}

/// Returns a free path in `dir` for `file_name`. On collision a timestamp is
/// inserted before the extension; the existing file is never overwritten.
/// `now_ms` is injected so the behaviour is testable.
pub fn unique_target(dir: &Path, file_name: &str, now_ms: i64) -> PathBuf {
    let direct = dir.join(file_name);
    if !direct.exists() {
        return direct;
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("datei");
    let ext = path.extension().and_then(|s| s.to_str());

    let mut n = 0;
    loop {
        let suffix = if n == 0 { format!("{now_ms}") } else { format!("{now_ms}-{n}") };
        let candidate = match ext {
            Some(e) => dir.join(format!("{stem}-{suffix}.{e}")),
            None => dir.join(format!("{stem}-{suffix}")),
        };
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

fn move_into(file: &Path, root: &Path, sub: &str, now_ms: i64) -> std::io::Result<PathBuf> {
    let dir = root.join(sub);
    std::fs::create_dir_all(&dir)?;
    let name = file.file_name().and_then(|s| s.to_str()).unwrap_or("datei");
    let target = unique_target(&dir, name, now_ms);
    std::fs::rename(file, &target)?;
    Ok(target)
}

/// Applies the folder's rule after a successful print.
pub fn apply_success(
    file: &Path, root: &Path, action: PostAction, now_ms: i64,
) -> std::io::Result<()> {
    match action {
        PostAction::Move => {
            move_into(file, root, PRINTED_DIR, now_ms)?;
        }
        PostAction::Keep => {}
        PostAction::Delete => std::fs::remove_file(file)?,
    }
    Ok(())
}

/// Applies the folder's rule after a job failed for good. A failed file is
/// never deleted — only moved aside when the folder moves files at all.
pub fn apply_failure(
    file: &Path, root: &Path, action: PostAction, now_ms: i64,
) -> std::io::Result<()> {
    if action == PostAction::Move {
        move_into(file, root, FAILED_DIR, now_ms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("printy_post_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn move_relocates_the_file_into_printed() {
        let d = temp("move");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_success(&f, &d, PostAction::Move, 1_700_000_000_000).unwrap();
        assert!(!f.exists());
        assert!(d.join(PRINTED_DIR).join("a.pdf").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn move_with_a_name_collision_appends_a_timestamp_and_never_overwrites() {
        let d = temp("collide");
        fs::create_dir_all(d.join(PRINTED_DIR)).unwrap();
        fs::write(d.join(PRINTED_DIR).join("a.pdf"), b"first").unwrap();
        let f = d.join("a.pdf");
        fs::write(&f, b"second").unwrap();

        apply_success(&f, &d, PostAction::Move, 1_700_000_000_000).unwrap();

        assert_eq!(
            fs::read(d.join(PRINTED_DIR).join("a.pdf")).unwrap(),
            b"first",
            "the existing file must survive untouched"
        );
        let n = fs::read_dir(d.join(PRINTED_DIR)).unwrap().count();
        assert_eq!(n, 2, "the new file lands beside it under a different name");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn keep_leaves_the_file_exactly_where_it_was() {
        let d = temp("keep");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_success(&f, &d, PostAction::Keep, 0).unwrap();
        assert!(f.exists());
        assert!(!d.join(PRINTED_DIR).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn delete_removes_the_file() {
        let d = temp("delete");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_success(&f, &d, PostAction::Delete, 0).unwrap();
        assert!(!f.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn failure_moves_to_failed_only_in_move_mode() {
        let d = temp("failed");
        let f = d.join("a.pdf");
        fs::write(&f, b"x").unwrap();
        apply_failure(&f, &d, PostAction::Move, 0).unwrap();
        assert!(d.join(FAILED_DIR).join("a.pdf").exists());

        let g = d.join("b.pdf");
        fs::write(&g, b"x").unwrap();
        apply_failure(&g, &d, PostAction::Delete, 0).unwrap();
        assert!(g.exists(), "a failed job is never deleted");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn unique_target_is_stable_when_there_is_no_collision() {
        let d = temp("unique");
        let t = unique_target(&d, "a.pdf", 1_700_000_000_000);
        assert_eq!(t, d.join("a.pdf"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn post_action_parses_permissively() {
        assert_eq!(PostAction::parse("keep"), PostAction::Keep);
        assert_eq!(PostAction::parse("delete"), PostAction::Delete);
        assert_eq!(PostAction::parse("banana"), PostAction::Move);
    }
}
