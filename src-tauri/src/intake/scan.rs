use std::path::{Path, PathBuf};

/// Reserved subdirectory names. They are never scanned — otherwise the watcher
/// rediscovers the file it just moved and prints in a loop.
pub const PRINTED_DIR: &str = "printed";
pub const FAILED_DIR: &str = "failed";

/// Lists printable candidates in `root`. Non-recursive by design.
pub fn scan_folder(root: &Path, types: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let wanted: Vec<String> = types.iter().map(|t| t.to_ascii_lowercase()).collect();
    let mut out = Vec::new();

    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if wanted.iter().any(|w| *w == ext) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("printy_scan_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn returns_only_files_with_configured_extensions() {
        let d = temp("ext");
        fs::write(d.join("a.pdf"), b"x").unwrap();
        fs::write(d.join("b.PDF"), b"x").unwrap();
        fs::write(d.join("c.txt"), b"x").unwrap();
        let mut got: Vec<String> = scan_folder(&d, &["pdf".into()])
            .unwrap().iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        got.sort();
        assert_eq!(got, vec!["a.pdf", "b.PDF"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn never_descends_into_printed_or_failed() {
        let d = temp("reserved");
        fs::create_dir_all(d.join(PRINTED_DIR)).unwrap();
        fs::create_dir_all(d.join(FAILED_DIR)).unwrap();
        fs::write(d.join(PRINTED_DIR).join("old.pdf"), b"x").unwrap();
        fs::write(d.join(FAILED_DIR).join("bad.pdf"), b"x").unwrap();
        fs::write(d.join("new.pdf"), b"x").unwrap();
        let got = scan_folder(&d, &["pdf".into()]).unwrap();
        assert_eq!(got.len(), 1, "printed/ and failed/ must never be rescanned");
        assert!(got[0].ends_with("new.pdf"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ignores_subdirectories_because_scanning_is_not_recursive() {
        let d = temp("norecurse");
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::write(d.join("sub").join("deep.pdf"), b"x").unwrap();
        assert!(scan_folder(&d, &["pdf".into()]).unwrap().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_folder_is_an_error_not_an_empty_list() {
        let d = std::env::temp_dir().join("printy_scan_absent_does_not_exist");
        let _ = std::fs::remove_dir_all(&d);
        assert!(scan_folder(&d, &["pdf".into()]).is_err());
    }
}
