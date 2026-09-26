//! PixivFlow Desktop — Tauri shell library crate.

pub mod backend;
mod commands;
pub mod config;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use backend::manager::BackendManager;
use config::AppConfig;
use tauri::Manager;

/// Minimal append-only file logger -> `logs/desktop.log` (mirrored to stdout so
/// it is visible during `cargo tauri dev`). No external logging dependency.
pub struct Logger {
    file: Mutex<BufWriter<File>>,
    path: String,
}

impl Logger {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let p = PathBuf::from(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)?;
        Ok(Self { file: Mutex::new(BufWriter::new(file)), path: path.to_string() })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn log(&self, level: &str, msg: &str) {
        let line = format!("{} [{level}] {msg}\n", format_unix_ts(unix_now()));
        print!("{line}"); // dev visibility
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }

    pub fn info(&self, m: &str) { self.log("INFO", m); }
    pub fn warn(&self, m: &str) { self.log("WARN", m); }
    pub fn error(&self, m: &str) { self.log("ERROR", m); }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn format_unix_ts(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let rem = unix % 86_400;
    let (y, mo, d) = civil_from_days(days);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let mo = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
}

/// Shared application state, managed by Tauri so commands can reach it.
pub struct ManagedState {
    pub manager: Mutex<BackendManager>,
    pub config_path: String,
    pub config_error: Mutex<Option<String>>,
    pub log: Logger,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_config,
            commands::config_path,
            commands::start_backend,
            commands::stop_backend,
            commands::restart_backend,
            commands::backend_doctor,
            commands::open_logs,
        ])
        .setup(|app| {
            // 1. config
            let cfg_result = config::load_or_create(app.handle());
            let (cfg, config_error) = match cfg_result {
                Ok((c, _path)) => (c, None),
                Err(e) => {
                    eprintln!("config load error: {e}");
                    println!("config load error: {e}");
                    (AppConfig::default(), Some(e))
                }
            };
            let config_path = config::config_path(app.handle())
                .unwrap_or_else(|_| PathBuf::from("desktop-config.json"));

            // 2. logs
            let log_path = if !cfg.log_dir.is_empty() {
                format!("{}/desktop.log", cfg.log_dir.trim_end_matches('/'))
            } else {
                "logs/desktop.log".to_string()
            };
            let logger = Logger::new(&log_path)
                .unwrap_or_else(|_| Logger::new("logs/desktop.log").expect("logger init"));

            let state = ManagedState {
                manager: Mutex::new(BackendManager::new(cfg.clone())),
                config_path: config_path.display().to_string(),
                config_error: Mutex::new(config_error),
                log: logger,
            };
            state.log.info(&format!(
                "==== PixivFlow Desktop F1 started ==== (mode={} port={})",
                cfg.mode,
                cfg.port()
            ));
            state.log.info(&format!("config: {}", state.config_path));
            if let Some(cfg_err) = &*state.config_error.lock().unwrap() {
                state.log.error(&format!("{cfg_err} (使用默认配置)"));
            }

            app.manage(state);

            // 3. auto-start the local backend in the background
            let app_handle = app.handle().clone();
            if cfg.is_local() && cfg.backend.auto_start {
                std::thread::spawn(move || {
                    {
                        let state = app_handle.state::<ManagedState>();
                        commands::apply_discovery(&state);
                        let start_res = state.manager.lock().unwrap().start();
                        match start_res {
                            Ok(pid) => {
                                state.log.info(&format!("auto-start: backend pid={pid}"))
                            }
                            Err(e) => {
                                state.log.error(&format!("auto-start failed: {e}"))
                            }
                        }
                    }
                    commands::poll_health_for(&app_handle);
                });
            }

            Ok(())
        })

        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let h = window.app_handle().clone();
                std::thread::spawn(move || {
                    if let Some(state) = h.try_state::<ManagedState>() {
                        match state.manager.lock().unwrap().stop() {
                            Ok(()) => state.log.info("window closed: backend stopped (graceful)"),
                            Err(e) => state.log.warn(&format!("window closed: stop backend failed: {e}")),
                        }
                    }
                });
            }
        })

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
