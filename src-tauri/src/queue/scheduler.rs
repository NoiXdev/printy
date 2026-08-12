use crate::db::settings;
use crate::print::factory::backend;
use crate::queue::worker::{run_one, QueueOutcome};
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager};

/// How often a held queue re-probes the printer.
pub const PROBE_INTERVAL_MS: i64 = 60_000;
const TICK_MS: u64 = 500;

/// Runtime-only state. Never persisted — unlike the user's pause switch, a
/// printer hold must clear itself when the printer comes back.
#[derive(Debug, Default)]
pub struct PrinterHold {
    reason: Option<String>,
    next_probe_ms: i64,
}

impl PrinterHold {
    pub fn engage(&mut self, reason: &str, now_ms: i64) {
        self.reason = Some(reason.to_string());
        self.next_probe_ms = now_ms + PROBE_INTERVAL_MS;
    }
    pub fn release(&mut self) {
        self.reason = None;
        self.next_probe_ms = 0;
    }
    pub fn is_due(&self, now_ms: i64) -> bool {
        self.reason.is_none() || now_ms >= self.next_probe_ms
    }
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The single queue worker. Jobs print strictly one after another — printing in
/// parallel would interleave the pages of two documents on one printer.
pub fn spawn_queue_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut hold = PrinterHold::default();
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(TICK_MS));

        loop {
            ticker.tick().await;
            // Clone out of State before any await: the guard is not Send.
            let db = {
                let state = app.state::<AppState>();
                state.db.clone()
            };

            if settings::user_paused(&db).await {
                continue;
            }
            let now = now_ms();
            if !hold.is_due(now) {
                continue;
            }

            let sumatra = settings::sumatra_path(&db).await;
            let db_for_blocking = db.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                let b = backend(sumatra);
                tauri::async_runtime::block_on(run_one(&db_for_blocking, b.as_ref(), now))
            })
            .await;

            let outcome = match outcome {
                Ok(Ok(o)) => o,
                _ => continue,
            };

            match &outcome {
                QueueOutcome::Idle => {}
                QueueOutcome::PrinterHold(reason) => {
                    hold.engage(reason, now);
                    let _ = app.emit("printy://queue", serde_json::json!({
                        "held": true, "reason": reason,
                    }));
                }
                _ => {
                    if hold.reason().is_some() {
                        hold.release();
                        let _ = app.emit("printy://queue",
                            serde_json::json!({ "held": false }));
                    }
                    let _ = app.emit("printy://job", serde_json::json!({
                        "outcome": format!("{outcome:?}"),
                    }));
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_state_reports_when_the_printer_should_be_probed_again() {
        let mut h = PrinterHold::default();
        assert!(h.is_due(0));

        h.engage("Drucker offline", 10_000);
        assert!(!h.is_due(10_000 + PROBE_INTERVAL_MS - 1));
        assert!(h.is_due(10_000 + PROBE_INTERVAL_MS));
        assert_eq!(h.reason(), Some("Drucker offline"));

        h.release();
        assert!(h.is_due(0));
        assert_eq!(h.reason(), None);
    }
}
