//! Tauri commands exposed to the desktop shell (and future settings window).
//! The WebUI window itself loads an external origin and cannot call these;
//! they are for native shell features and programmatic control.

use crate::backend::BackendStatus;
use crate::config::{save_config, AppConfig};
use crate::AppState;
use tauri::State;

/// Current backend/app status snapshot.
#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> BackendStatus {
    {
        let mut mgr = state.manager.lock().unwrap();
        mgr.poll_exit();
    }
    state.manager.lock().unwrap().status()
}

/// Reads the desktop config (as loaded at startup).
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.manager.lock().unwrap().cfg.clone()
}

/// Persists the desktop config to `config.json`.
///
/// For MVP this does not hot-apply backend settings; the backend is restarted
/// manually (or on next launch). Future iterations can diff and restart.
#[tauri::command]
pub fn save_config(state: State<'_, AppState>, cfg: AppConfig) -> Result<AppConfig, String> {
    let st = state.inner();
    let mut mgr = st.manager.lock().unwrap();
    mgr.cfg = cfg.clone();
    save_config(&cfg, &st.config_path).map_err(|e| format!("failed to write config.json: {e}"))?;
    Ok(cfg)
}

/// Starts the local backend and waits until it becomes healthy.
#[tauri::command]
pub fn start_backend(state: State<'_, AppState>) -> Result<BackendStatus, String> {
    let mut mgr = state.manager.lock().unwrap();
    let st_after_start = mgr.start()?;
    if mgr.wait_healthy().is_err() {
        let err = mgr.last_error.clone().unwrap_or_else(|| "backend failed".to_string());
        return Err(err);
    }
    Ok(st_after_start)
}

/// Gracefully stops the backend.
#[tauri::command]
pub fn stop_backend(state: State<'_, AppState>) -> Result<BackendStatus, String> {
    let mut mgr = state.manager.lock().unwrap();
    mgr.stop()
}

/// Restarts the backend (graceful stop, then start + wait healthy).
#[tauri::command]
pub fn restart_backend(state: State<'_, AppState>) -> Result<BackendStatus, String> {
    let mut mgr = state.manager.lock().unwrap();
    mgr.restart()
}