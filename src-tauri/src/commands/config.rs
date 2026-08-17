use crate::config::export::export_config;
use crate::config::import::{import_config, ImportMode, ImportReport};
use crate::config::schema::{parse_export, ConfigExport};
use crate::db::settings;
use crate::print::factory::backend;
use crate::{error::AppError, error::AppResult, AppState};
use std::collections::HashSet;
use tauri::State;

/// Returns the exported configuration as a pretty-printed JSON string. The
/// frontend gets the destination path from the native save dialog and then
/// writes the string itself through `write_text_file_cmd` -- the same
/// dialog-picks-a-path / command-does-the-I/O split already used for folder
/// picking (`FolderDialog`'s directory picker, the pdfium browse button).
#[tauri::command]
pub async fn export_config_cmd(state: State<'_, AppState>) -> AppResult<String> {
    let export = export_config(&state.db).await?;
    serde_json::to_string_pretty(&export).map_err(|e| {
        AppError::Other(format!("Konfiguration konnte nicht erzeugt werden: {e}"))
    })
}

/// Parses, validates, and applies an exported configuration. The current
/// printer list is fetched here (the one place that actually talks to the
/// print subsystem) so `config::import::import_config` itself stays testable
/// with a plain in-memory printer set.
#[tauri::command]
pub async fn import_config_cmd(
    state: State<'_, AppState>,
    json: String,
    mode: ImportMode,
) -> AppResult<ImportReport> {
    let export: ConfigExport = parse_export(&json).map_err(|e| AppError::Other(e.to_string()))?;

    let sumatra = settings::sumatra_path(&state.db).await;
    let pdfium = settings::pdfium_path(&state.db).await;
    let printers = tokio::task::spawn_blocking(move || backend(sumatra, pdfium).list_printers())
        .await
        .map_err(|e| AppError::Other(format!("Druckerliste: {e}")))?
        .map_err(|e| AppError::Print(e.message))?;
    let available: HashSet<String> = printers.into_iter().map(|p| p.name).collect();

    Ok(import_config(&state.db, export, mode, &available).await?)
}
