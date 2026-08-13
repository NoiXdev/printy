use crate::db::settings;
use crate::{error::AppResult, AppState};
use std::collections::HashMap;
use tauri::State;

const KEYS: &[(&str, &str)] = &[
    ("notification_mode", "all"),
    ("autostart", "0"),
    ("start_minimized", "0"),
    ("sumatra_path", ""),
    ("pdfium_path", ""),
    ("default_poll_interval_secs", "3"),
    ("user_paused", "0"),
];

#[tauri::command]
pub async fn get_settings_cmd(state: State<'_, AppState>) -> AppResult<HashMap<String, String>> {
    let mut out = HashMap::new();
    for (key, default) in KEYS {
        let v = settings::get_setting(&state.db, key)
            .await?
            .unwrap_or_else(|| default.to_string());
        out.insert(key.to_string(), v);
    }
    Ok(out)
}

#[tauri::command]
pub async fn update_setting_cmd(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    if !KEYS.iter().any(|(k, _)| *k == key) {
        return Err(crate::error::AppError::Other(format!(
            "Unbekannte Einstellung: {key}"
        )));
    }
    Ok(settings::set_setting(&state.db, &key, &value).await?)
}

#[tauri::command]
pub async fn set_global_paused_cmd(state: State<'_, AppState>, paused: bool) -> AppResult<()> {
    Ok(settings::set_setting(&state.db, "user_paused", if paused { "1" } else { "0" }).await?)
}
