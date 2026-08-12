use crate::db::settings;
use crate::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager};

pub const TRAY_ID: &str = "printy-tray";
const ICON_SIZE: u32 = 32;
const UPDATE_INTERVAL_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Running,
    Error,
    Paused,
}

impl TrayState {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            // Mint means running, coral means something needs attention, grey
            // means the user switched it off. Same three roles as the folder dots.
            TrayState::Running => [0x2f, 0xe6, 0xb7],
            TrayState::Error => [0xff, 0x7a, 0x59],
            TrayState::Paused => [0x9a, 0xa3, 0xa1],
        }
    }
}

/// The user's own pause outranks a stuck printer: a deliberately stopped Printy
/// must not shout. Beyond that the icon answers exactly one question — is the
/// queue held? A single failed file is not a tray-level event; it is a row in
/// Verlauf.
pub fn tray_state(user_paused: bool, queue_held: bool) -> TrayState {
    if user_paused {
        return TrayState::Paused;
    }
    if queue_held {
        return TrayState::Error;
    }
    TrayState::Running
}

/// Draws the Printy folder silhouette as raw RGBA. Generated rather than loaded
/// so the state colour is a constant, not three binary assets, and so the shape
/// is testable.
pub fn tray_rgba(state: TrayState) -> Vec<u8> {
    let [r, g, b] = state.rgb();
    let mut px = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let in_tab = (3..14).contains(&x) && (7..11).contains(&y);
            let in_body = (3..29).contains(&x) && (10..26).contains(&y);
            if !in_tab && !in_body {
                continue;
            }
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            px[i] = r;
            px[i + 1] = g;
            px[i + 2] = b;
            px[i + 3] = 0xff;
        }
    }
    px
}

fn icon_for(state: TrayState) -> Image<'static> {
    Image::new_owned(tray_rgba(state), ICON_SIZE, ICON_SIZE)
}

/// Shows the main window and brings it to the front.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Toggles the main window's visibility (for the tray icon click).
pub fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", "Öffnen", true, None::<&str>)?;
    let pause_i = MenuItem::with_id(app, "pause", "Pause", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &pause_i, &quit_i])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon_for(TrayState::Paused))
        .tooltip("Printy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "pause" => {
                // Flip the persisted global switch. Cloned out of State before
                // the await, because the guard is not Send.
                let handle = app.clone();
                let db = { handle.state::<AppState>().db.clone() };
                tauri::async_runtime::spawn(async move {
                    let now = settings::user_paused(&db).await;
                    let _ = settings::set_setting(
                        &db,
                        "user_paused",
                        if now { "0" } else { "1" },
                    )
                    .await;
                    let _ = handle.emit(
                        "printy://queue",
                        serde_json::json!({ "held": false, "paused": !now }),
                    );
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Keeps the tray colour in step with reality. Only two inputs matter: the
/// persisted global pause, and the printer hold — runtime-only state that never
/// reaches the database, so it is picked up from the very event the queue
/// scheduler already emits.
pub fn spawn_tray_updater(app: AppHandle) {
    let held = Arc::new(AtomicBool::new(false));

    let held_for_listener = held.clone();
    app.listen("printy://queue", move |event| {
        let is_held = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("held").and_then(|h| h.as_bool()))
            .unwrap_or(false);
        held_for_listener.store(is_held, Ordering::Relaxed);
    });

    tauri::async_runtime::spawn(async move {
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_millis(UPDATE_INTERVAL_MS));
        let mut last: Option<TrayState> = None;

        loop {
            ticker.tick().await;
            let db = {
                let state = app.state::<AppState>();
                state.db.clone()
            };

            let paused = settings::user_paused(&db).await;
            let next = tray_state(paused, held.load(Ordering::Relaxed));
            if last == Some(next) {
                continue;
            }
            last = Some(next);
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_icon(Some(icon_for(next)));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_users_pause_outranks_a_stuck_printer() {
        assert_eq!(tray_state(true, true), TrayState::Paused);
        assert_eq!(tray_state(true, false), TrayState::Paused);
    }

    #[test]
    fn a_held_queue_is_the_only_thing_that_turns_the_icon_coral() {
        assert_eq!(tray_state(false, true), TrayState::Error);
    }

    #[test]
    fn everything_running_shows_mint() {
        assert_eq!(tray_state(false, false), TrayState::Running);
    }

    #[test]
    fn each_state_carries_its_own_brand_colour() {
        assert_eq!(TrayState::Running.rgb(), [0x2f, 0xe6, 0xb7]);
        assert_eq!(TrayState::Error.rgb(), [0xff, 0x7a, 0x59]);
        assert_eq!(TrayState::Paused.rgb(), [0x9a, 0xa3, 0xa1]);
    }

    #[test]
    fn the_icon_is_a_full_rgba_buffer_of_the_declared_size() {
        let px = tray_rgba(TrayState::Running);
        assert_eq!(px.len() as u32, ICON_SIZE * ICON_SIZE * 4);
    }

    #[test]
    fn the_folder_body_is_opaque_and_the_corners_are_transparent() {
        let px = tray_rgba(TrayState::Error);
        let at = |x: u32, y: u32| {
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        assert_eq!(at(16, 18), [0xff, 0x7a, 0x59, 0xff]);
        assert_eq!(at(0, 0)[3], 0);
        assert_eq!(at(31, 31)[3], 0);
    }

    #[test]
    fn the_tab_above_the_body_is_drawn_too() {
        let px = tray_rgba(TrayState::Paused);
        let i = ((8 * ICON_SIZE + 6) * 4) as usize;
        assert_eq!(px[i + 3], 0xff);
    }

    #[test]
    fn different_states_produce_different_pixels() {
        assert_ne!(tray_rgba(TrayState::Running), tray_rgba(TrayState::Paused));
    }
}
