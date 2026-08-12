use crate::db::settings;
use crate::intake::scan::scan_folder;
use crate::{error::AppError, error::AppResult, AppState};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

/// Counts the files a freshly created folder would print. Uses the same
/// listing the watcher uses, so `printed/` and `failed/` are excluded and the
/// type filter matches exactly what will actually be picked up. A folder that
/// does not exist yet counts as zero rather than failing — the dialog must
/// stay usable while the user is still typing a path.
pub fn count_existing(path: &str, file_types: &[String]) -> std::io::Result<usize> {
    let root = std::path::Path::new(path);
    if !root.is_dir() {
        return Ok(0);
    }
    Ok(scan_folder(root, file_types)?.len())
}

#[tauri::command]
pub async fn count_existing_files_cmd(
    _state: State<'_, AppState>,
    path: String,
    file_types: Vec<String>,
) -> AppResult<usize> {
    Ok(count_existing(&path, &file_types)?)
}

/// Reports what the operating system believes, not what the database stored.
/// A user who removed the login item by hand must see the switch turn itself off.
#[tauri::command]
pub async fn get_autostart_cmd(app: AppHandle) -> AppResult<bool> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Other(format!("Autostart nicht lesbar: {e}")))
}

/// Persists the setting *and* applies it, so the change takes effect now rather
/// than after the next launch.
#[tauri::command]
pub async fn set_autostart_cmd(
    state: State<'_, AppState>,
    app: AppHandle,
    enabled: bool,
) -> AppResult<()> {
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| AppError::Other(format!("Autostart nicht änderbar: {e}")))?;
    settings::set_setting(&state.db, "autostart", if enabled { "1" } else { "0" }).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Temp directory with a uniquifier and best-effort cleanup, as in
    /// `autofetch/local.rs` — no `tempfile` dev-dependency is added.
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("printy-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn counts_only_files_matching_the_type_filter() {
        let dir = temp_dir("count-types");
        fs::write(dir.join("a.pdf"), b"x").unwrap();
        fs::write(dir.join("b.PDF"), b"x").unwrap();
        fs::write(dir.join("c.png"), b"x").unwrap();
        fs::write(dir.join("d.docx"), b"x").unwrap();

        let types = vec!["pdf".to_string()];
        assert_eq!(count_existing(dir.to_str().unwrap(), &types).unwrap(), 2);

        let types = vec!["pdf".to_string(), "png".to_string()];
        assert_eq!(count_existing(dir.to_str().unwrap(), &types).unwrap(), 3);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ignores_the_reserved_subdirectories() {
        let dir = temp_dir("count-reserved");
        fs::write(dir.join("a.pdf"), b"x").unwrap();
        fs::create_dir_all(dir.join("printed")).unwrap();
        fs::write(dir.join("printed").join("old.pdf"), b"x").unwrap();
        fs::create_dir_all(dir.join("failed")).unwrap();
        fs::write(dir.join("failed").join("bad.pdf"), b"x").unwrap();

        let types = vec!["pdf".to_string()];
        assert_eq!(count_existing(dir.to_str().unwrap(), &types).unwrap(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_directory_counts_as_zero_not_an_error() {
        let missing = std::env::temp_dir().join("printy-does-not-exist-9999");
        let types = vec!["pdf".to_string()];
        assert_eq!(count_existing(missing.to_str().unwrap(), &types).unwrap(), 0);
    }
}
