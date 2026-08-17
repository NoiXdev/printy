mod commands;
mod config;
mod db;
mod error;
mod intake;
mod print;
mod queue;
mod shell;
mod watcher;

use tauri::Manager;

pub struct AppState {
    pub db: db::Db,
    pub app_data: std::path::PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin registered: it needs to intercept the
        // process before anything else claims the single SQLite file and
        // spawns a second queue worker / watcher set against it.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            shell::tray::show_main(app);
        }))
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

            shell::tray::setup_tray(app)?;
            shell::tray::spawn_tray_updater(app.handle().clone());

            // The window starts hidden (tauri.conf.json), so autostart lands in
            // the tray. Show it unless the user asked for a minimised start.
            let start_min = tauri::async_runtime::block_on(db::settings::start_minimized(
                &app.state::<AppState>().db,
            ));
            if !start_min {
                shell::tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing puts Printy in the tray instead of quitting it — the whole
            // point of the app is that it keeps watching.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::export_config_cmd,
            commands::config::import_config_cmd,
            commands::folders::list_folders_cmd,
            commands::folders::create_folder_cmd,
            commands::folders::update_folder_cmd,
            commands::folders::delete_folder_cmd,
            commands::folders::folder_delete_impact_cmd,
            commands::folders::set_folder_enabled_cmd,
            commands::folders::scan_now_cmd,
            commands::folders::scan_all_folders_cmd,
            commands::jobs::get_status_cmd,
            commands::jobs::list_jobs_cmd,
            commands::jobs::reprint_job_cmd,
            commands::printers::list_printers_cmd,
            commands::printers::printer_capabilities_cmd,
            commands::settings::get_settings_cmd,
            commands::settings::update_setting_cmd,
            commands::settings::set_global_paused_cmd,
            commands::shell::count_existing_files_cmd,
            commands::shell::get_autostart_cmd,
            commands::shell::set_autostart_cmd,
            commands::shell::write_text_file_cmd,
            commands::shell::read_text_file_cmd,
            commands::shell::get_app_version_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
