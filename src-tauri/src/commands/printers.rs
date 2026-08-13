use crate::db::settings;
use crate::print::factory::backend;
use crate::print::{PrinterCapabilities, PrinterInfo};
use crate::{error::AppError, error::AppResult, AppState};
use tauri::State;

#[tauri::command]
pub async fn list_printers_cmd(state: State<'_, AppState>) -> AppResult<Vec<PrinterInfo>> {
    let sumatra = settings::sumatra_path(&state.db).await;
    let pdfium = settings::pdfium_path(&state.db).await;
    tokio::task::spawn_blocking(move || backend(sumatra, pdfium).list_printers())
        .await
        .map_err(|e| AppError::Other(format!("Druckerliste: {e}")))?
        .map_err(|e| AppError::Print(e.message))
}

#[tauri::command]
pub async fn printer_capabilities_cmd(
    state: State<'_, AppState>,
    printer: String,
) -> AppResult<PrinterCapabilities> {
    let sumatra = settings::sumatra_path(&state.db).await;
    let pdfium = settings::pdfium_path(&state.db).await;
    tokio::task::spawn_blocking(move || backend(sumatra, pdfium).capabilities(&printer))
        .await
        .map_err(|e| AppError::Other(format!("Druckerfähigkeiten: {e}")))?
        .map_err(|e| AppError::Print(e.message))
}
