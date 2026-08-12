use crate::db::folders::{self, NewFolder};
use crate::db::models::WatchFolder;
use crate::intake::stability::StabilityTracker;
use crate::watcher::tick::{mark_existing_as_seen, tick_folder};
use crate::{error::AppResult, AppState};
use tauri::State;

#[tauri::command]
pub async fn list_folders_cmd(state: State<'_, AppState>) -> AppResult<Vec<WatchFolder>> {
    Ok(folders::list_folders(&state.db).await?)
}

/// `print_existing = false` marks everything already in the folder as handled.
/// This is the safe default; the opposite must be chosen explicitly.
#[tauri::command]
pub async fn create_folder_cmd(
    state: State<'_, AppState>,
    folder: NewFolder,
    print_existing: bool,
) -> AppResult<WatchFolder> {
    let created = folders::create_folder(&state.db, &folder).await?;
    if !print_existing {
        mark_existing_as_seen(&state.db, &created).await?;
    }
    Ok(created)
}

#[tauri::command]
pub async fn update_folder_cmd(
    state: State<'_, AppState>,
    id: i64,
    folder: NewFolder,
) -> AppResult<WatchFolder> {
    Ok(folders::update_folder(&state.db, id, &folder).await?)
}

#[tauri::command]
pub async fn delete_folder_cmd(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    Ok(folders::delete_folder(&state.db, id).await?)
}

#[tauri::command]
pub async fn set_folder_enabled_cmd(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> AppResult<()> {
    Ok(folders::set_folder_enabled(&state.db, id, enabled).await?)
}

/// Runs two ticks back to back so a settled file is picked up immediately
/// rather than waiting for the next interval.
#[tauri::command]
pub async fn scan_now_cmd(state: State<'_, AppState>, id: i64) -> AppResult<usize> {
    let Some(folder) = folders::get_folder(&state.db, id).await? else {
        return Err(crate::error::AppError::Other("Ordner nicht gefunden".into()));
    };
    let mut tracker = StabilityTracker::new();
    tick_folder(&state.db, &folder, &mut tracker).await?;
    let report = tick_folder(&state.db, &folder, &mut tracker).await?;
    Ok(report.enqueued)
}
