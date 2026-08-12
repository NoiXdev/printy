use crate::db::folders;
use crate::intake::stability::StabilityTracker;
use crate::watcher::tick::tick_folder;
use crate::AppState;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};

const SUPERVISOR_INTERVAL_SECS: u64 = 1;

/// A single supervisor task drives every folder on its own interval. One task
/// per folder would need respawning on every configuration change; this does
/// not, and the work per tick is a directory listing.
pub fn spawn_watchers(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut trackers: HashMap<i64, StabilityTracker> = HashMap::new();
        let mut last_run: HashMap<i64, i64> = HashMap::new();
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(SUPERVISOR_INTERVAL_SECS));

        loop {
            ticker.tick().await;
            let db = {
                let state = app.state::<AppState>();
                state.db.clone()
            };
            if crate::db::settings::user_paused(&db).await {
                continue;
            }

            let now = chrono::Utc::now().timestamp();
            let all = folders::list_folders(&db).await.unwrap_or_default();
            let live: Vec<i64> = all.iter().map(|f| f.id).collect();
            trackers.retain(|id, _| live.contains(id));
            last_run.retain(|id, _| live.contains(id));

            for folder in all.into_iter().filter(|f| f.enabled == 1) {
                let due = last_run
                    .get(&folder.id)
                    .map(|t| now - t >= folder.poll_interval_secs)
                    .unwrap_or(true);
                if !due {
                    continue;
                }
                last_run.insert(folder.id, now);

                let tracker = trackers.entry(folder.id).or_insert_with(StabilityTracker::new);
                if let Ok(report) = tick_folder(&db, &folder, tracker).await {
                    // `status_changed`, not `path_missing`: a folder that stays
                    // gone must notify once, not once per second.
                    if report.enqueued > 0 || report.status_changed {
                        let _ = app.emit("printy://folder", serde_json::json!({
                            "folder_id": folder.id,
                            "enqueued": report.enqueued,
                            "path_missing": report.path_missing,
                        }));
                    }
                }
            }
        }
    });
}
