mod commands;
mod db;
mod error;
mod intake;
mod print;
mod queue;
mod watcher;

use tauri::Manager;

pub struct AppState {
    pub db: db::Db,
    pub app_data: std::path::PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&dir).ok();
            let db_path = dir.join("printy.sqlite");
            let url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
            let db = tauri::async_runtime::block_on(db::connect(&url))
                .expect("failed to connect/migrate database");

            // A job left mid-print means the app died. Fail it; never reprint.
            let recovered =
                tauri::async_runtime::block_on(db::jobs::recover_interrupted(&db)).unwrap_or(0);
            if recovered > 0 {
                eprintln!("recovered {recovered} interrupted job(s)");
            }

            app.manage(AppState { db, app_data: dir });
            watcher::scheduler::spawn_watchers(app.handle().clone());
            queue::scheduler::spawn_queue_worker(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folders::list_folders_cmd,
            commands::folders::create_folder_cmd,
            commands::folders::update_folder_cmd,
            commands::folders::delete_folder_cmd,
            commands::folders::set_folder_enabled_cmd,
            commands::folders::scan_now_cmd,
            commands::jobs::get_status_cmd,
            commands::jobs::list_jobs_cmd,
            commands::jobs::reprint_job_cmd,
            commands::printers::list_printers_cmd,
            commands::printers::printer_capabilities_cmd,
            commands::settings::get_settings_cmd,
            commands::settings::update_setting_cmd,
            commands::settings::set_global_paused_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
