//! Tauri commands (F1). Rust owns system capability; the frontend only
//! invokes these and renders the resulting status.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::backend::discovery::{self, DoctorReport};
use crate::backend::{BackendSource, StatusSnapshot};
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
    apply_discovery(&state);
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
    apply_discovery(&state);
    let pid = state.manager.lock().unwrap().restart()?;
    state.log.info(&format!("backend restarted pid={pid}"));
    poll_health(&state, &app, 15);
    push_status(&app, &state);
    Ok(snapshot(&state))
}


/// Apply the discovery adapter to the manager so `start()` runs the resolved
/// real command (bundled / config / PATH) instead of the plain mock default.
/// Returns the descriptor plus whether a REAL PixivFlow was found.
pub fn apply_discovery(state: &ManagedState) -> (discovery::BackendDescriptor, bool) {
    let cfg = state.manager.lock().unwrap().config_clone();
    let d = discovery::discover(&cfg);
    let cmd = if d.command.is_empty() {
        None
    } else {
        Some((d.command[0].clone(), d.command[1..].to_vec()))
    };
    state.manager.lock().unwrap().set_command_override(cmd);
    let real = d.is_real();
    state.log.info(&format!(
        "backend resolved: source={} exe={:?} real={real}",
        d.source.as_str(),
        d.executable_path
    ));
    (d, real)
}

/// F2.1 diagnostic: what backend did discovery resolve, and is it healthy?
#[tauri::command]
pub fn backend_doctor(state: State<'_, ManagedState>) -> DoctorReport {
    let cfg = state.manager.lock().unwrap().config_clone();
    let d = discovery::discover_with_version(&cfg);
    let running = state.manager.lock().unwrap().is_running();
    let (healthy, message) = if running {
        match state.manager.lock().unwrap().health_check() {
            Ok(true) => (Some(true), "healthy (GET /api/health -> 200)".into()),
            Ok(false) => (Some(false), "running but not healthy (no 200 on /api/health)".into()),
            Err(e) => (None, format!("health probe failed: {e}")),
        }
    } else if matches!(d.source, BackendSource::Mock | BackendSource::NotFound) {
        (None, "not running — dev mock fallback (no real PixivFlow resolved)".into())
    } else {
        (None, "not running — backend resolved but not started".into())
    };
    DoctorReport {
        backend_found: d.source != BackendSource::NotFound,
        executable_path: d.executable_path,
        source: d.source.as_str().into(),
        version: d.version,
        port: cfg.port(),
        running,
        healthy,
        message,
    }
}


/// Open the desktop log file in the platform's default viewer.
#[tauri::command]
pub fn open_logs(state: State<'_, ManagedState>) -> Result<(), String> {
    let path = state.log.path().to_string();
    open_in_default_viewer(&path)
}

/// Spawn the OS file opener for `path`. No waiting; failures surface as Err.
fn open_in_default_viewer(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(path)
            .status()
            .map(|_| ())
            .map_err(|e| format!("failed to open log {path}: {e}"))
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "start", "", path])
            .status()
            .map(|_| ())
            .map_err(|e| format!("failed to open log {path}: {e}"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(path)
            .status()
            .map(|_| ())
            .map_err(|e| format!("failed to open log {path}: {e}"))
    }
}
