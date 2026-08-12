use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub mtime_ms: i64,
}

pub fn stamp(path: &Path) -> std::io::Result<FileStamp> {
    let md = std::fs::metadata(path)?;
    let mtime_ms = md
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(FileStamp { size: md.len(), mtime_ms })
}

/// Remembers the previous stamp per path. A file counts as complete only when
/// two consecutive observations are identical — the cheapest reliable way to
/// avoid printing a file that is still being written.
#[derive(Debug, Default)]
pub struct StabilityTracker {
    seen: HashMap<PathBuf, FileStamp>,
}

impl StabilityTracker {
    pub fn new() -> Self {
        StabilityTracker { seen: HashMap::new() }
    }

    /// Returns true when this stamp matches the one recorded on the last tick.
    pub fn observe(&mut self, path: &Path, current: FileStamp) -> bool {
        match self.seen.insert(path.to_path_buf(), current.clone()) {
            Some(prev) => prev == current,
            None => false,
        }
    }

    pub fn forget(&mut self, path: &Path) {
        self.seen.remove(path);
    }
}

/// A file that cannot be opened for reading is still held by its writer.
pub fn is_readable(path: &Path) -> bool {
    std::fs::File::open(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("printy_stab_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_file_is_not_stable_on_first_sight() {
        let d = temp("first");
        let f = d.join("a.pdf");
        fs::write(&f, b"12345").unwrap();
        let mut t = StabilityTracker::new();
        assert!(!t.observe(&f, stamp(&f).unwrap()));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_is_stable_on_the_second_identical_observation() {
        let d = temp("second");
        let f = d.join("a.pdf");
        fs::write(&f, b"12345").unwrap();
        let s = stamp(&f).unwrap();
        let mut t = StabilityTracker::new();
        t.observe(&f, s.clone());
        assert!(t.observe(&f, s));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_growing_file_never_becomes_stable() {
        let d = temp("growing");
        let f = d.join("a.pdf");
        let mut t = StabilityTracker::new();
        for i in 1..5 {
            fs::write(&f, vec![b'x'; i * 100]).unwrap();
            let s = FileStamp { size: (i * 100) as u64, mtime_ms: 1_000 + i as i64 };
            assert!(!t.observe(&f, s), "a file still being written must not print");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn same_size_but_newer_mtime_resets_stability() {
        let f = PathBuf::from("/tmp/printy-virtual.pdf");
        let mut t = StabilityTracker::new();
        t.observe(&f, FileStamp { size: 100, mtime_ms: 1_000 });
        assert!(!t.observe(&f, FileStamp { size: 100, mtime_ms: 2_000 }));
        assert!(t.observe(&f, FileStamp { size: 100, mtime_ms: 2_000 }));
    }

    #[test]
    fn forget_drops_tracking_state_for_a_path() {
        let f = PathBuf::from("/tmp/printy-virtual2.pdf");
        let s = FileStamp { size: 1, mtime_ms: 1 };
        let mut t = StabilityTracker::new();
        t.observe(&f, s.clone());
        t.forget(&f);
        assert!(!t.observe(&f, s), "after forget the file starts over");
    }

    #[test]
    fn stamp_reads_size_and_mtime_from_disk() {
        let d = temp("stamp");
        let f = d.join("a.pdf");
        fs::write(&f, b"hello").unwrap();
        let s = stamp(&f).unwrap();
        assert_eq!(s.size, 5);
        assert!(s.mtime_ms > 0);
        let _ = fs::remove_dir_all(&d);
    }
}
