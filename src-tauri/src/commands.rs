//! Tauri commands (F1). Rust owns system capability; the frontend only
//! invokes these and renders the resulting status.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::backend::StatusSnapshot;
use crate::config::AppConfig;
use crate::ManagedState;

fn push_status(app: &AppHandle, state: &ManagedState) {
    let snap = snapshot(state);
    let _ = app.emit("backend-status", snap);
}

/// Publish the current snapshot as a `backend-status` event (one-arg form
/// available to background code in lib.rs).
pub fn emit_status(app: &AppHandle) {
    let state = app.state::<ManagedState>();
    let snap = snapshot(&state);
    let _ = app.emit("backend-status", snap);
}

/// Poll health for the background auto-start path in lib.rs.
pub fn poll_health_for(app: &AppHandle) {
    let state = app.state::<ManagedState>();
    poll_health(&state, app, 15);
    emit_status(app);
}

fn snapshot(state: &ManagedState) -> StatusSnapshot {
    let m = state.manager.lock().unwrap();
    let running = m.is_running();
    let healthy = m.healthy();
    let err = m.last_error();
    let cfg_err = state.config_error.lock().unwrap().clone();
    let port = m.port();
    let pid = m.pid();

    let state_name = if cfg_err.is_some() {
        "error"
    } else if running {
        match healthy {
            Some(true) => "running",
            _ => "starting",
        }
    } else if err.is_some() {
        "error"
    } else {
        "stopped"
    };

    StatusSnapshot {
        state: state_name.into(),
        mode: m.mode().into(),
        port,
        pid,
        healthy: healthy.unwrap_or(false),
        message: cfg_err.or(err),
    }
}

/// Poll the health endpoint until it returns 200 or the attempt budget runs out.
/// Each probe result is pushed to the frontend as a `backend-status` event.
fn poll_health(state: &ManagedState, app: &AppHandle, attempts: u32) {
    for i in 0..attempts {
        let probe = {
            let m = state.manager.lock().unwrap();
            m.health_check()
        };
        match probe {
            Ok(true) => {
                state.manager.lock().unwrap().set_healthy(true);
                state.manager.lock().unwrap().report_err("");
                state.log.info("health OK (/api/health -> 200)");
                push_status(app, state);
                return;
            }
            Ok(false) => {
                state.manager.lock().unwrap().set_healthy(false);
            }
            Err(e) => {
                state.manager.lock().unwrap().report_err(&e);
                state.log.warn(&format!("health probe {i}: {e}"));
            }
        }
        push_status(app, state);
        std::thread::sleep(Duration::from_millis(400));
    }
    state.log.warn("health not confirmed within attempt budget");
    push_status(app, state);
}

#[tauri::command]
pub fn get_status(state: State<'_, ManagedState>) -> StatusSnapshot {
    snapshot(&state)
}

#[tauri::command]
pub fn get_config(state: State<'_, ManagedState>) -> AppConfig {
    state.manager.lock().unwrap().config_clone()
}

#[tauri::command]
pub fn config_path(state: State<'_, ManagedState>) -> String {
    state.config_path.clone()
}

#[tauri::command]
pub fn start_backend(
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<StatusSnapshot, String> {
    let port = state.manager.lock().unwrap().port();
    state.log.info(&format!("start_backend requested (port {port})"));
    let pid = state.manager.lock().unwrap().start()?;
    state.log.info(&format!("backend started pid={pid}"));
    poll_health(&state, &app, 15);
    push_status(&app, &state);
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn stop_backend(
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<StatusSnapshot, String> {
    state.manager.lock().unwrap().stop()?;
    state.log.info("backend stopped");
    push_status(&app, &state);
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn restart_backend(
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<StatusSnapshot, String> {
    let pid = state.manager.lock().unwrap().restart()?;
    state.log.info(&format!("backend restarted pid={pid}"));
    poll_health(&state, &app, 15);
    push_status(&app, &state);
    Ok(snapshot(&state))
}
