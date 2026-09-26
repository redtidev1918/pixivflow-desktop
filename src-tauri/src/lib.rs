pub mod backend;
pub mod commands;
pub mod config;
pub mod health;
pub mod logging;

use backend::{BackendManager, Paths};
use config::AppConfig;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use tauri::{AppHandle, Manager, RunEvent, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// Global, app-managed state. Thin wrapper around the BackendManager so it can
/// be shared across Tauri commands, the boot thread and window-lifecycle hooks.
pub struct AppState {
    pub manager: Mutex<BackendManager>,
    pub config_path: PathBuf,
    /// Set once when we begin tearing the backend down at exit, so that the
    /// close/exit hooks never double-stop.
    pub stopping: Arc<AtomicBool>,
}

const MAIN_WINDOW: &str = "main";
const BOOT_WINDOW: &str = "boot";

pub fn run() {
    let app = tauri::Builder::default()
        // Only one desktop instance; a second launch focuses the existing app.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = focus_any_window(app);
        }))
        .setup(|app| {
            let app_handle = app.handle().clone();
            setup_state(&app_handle)?;
            // Launch the backend (if configured) and open the main window on a
            // background thread so the event loop stays responsive.
            std::thread::spawn(move || boot(&app_handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_config,
            commands::save_config,
            commands::start_backend,
            commands::stop_backend,
            commands::restart_backend,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == MAIN_WINDOW || window.label() == BOOT_WINDOW {
                    // Closing the webui window should quit the desktop app and
                    // gracefully stop the backend.
                    api.prevent_close();
                    let app = window.app_handle().clone();
                    std::thread::spawn(move || {
                        stop_backend(&app);
                        app.exit(0);
                    });
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building the PixivFlow Desktop application");

    // Safety net: no matter how the app terminates (Cmd+Q on macOS, system
    // shutdown, crash-free exit), stop the backend gracefully.
    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            stop_backend(handle);
        }
    });
}

/// Builds and manages the app state (config, paths, backend manager, logger).
fn setup_state(app: &AppHandle) -> tauri::Result<()> {
    let paths = resolve_paths(app);
    if let Err(e) = std::fs::create_dir_all(&paths.data_dir) {
        logging::warn(&format!("could not ensure data dir: {e}"));
    }
    let _ = std::fs::create_dir_all(&paths.logs_dir);
    logging::init(&paths.logs_dir);

    let config_dir = app.path().app_config_dir()?;
    let (cfg, cfg_path) = match config::load_config(&config_dir) {
        Ok(x) => x,
        Err(e) => {
            logging::error(&format!("failed to load config, using defaults: {e}"));
            (AppConfig::default(), config_dir.join("config.json"))
        }
    };

    let paths = Paths {
        data_dir: resolve_non_empty(&cfg.data_dir, paths.data_dir),
        download_dir: resolve_non_empty(&cfg.download_dir, paths.download_dir),
        logs_dir: resolve_non_empty(&cfg.logs_dir, paths.logs_dir),
        resource_dir: paths.resource_dir,
    };
    let _ = std::fs::create_dir_all(&paths.data_dir);
    let _ = std::fs::create_dir_all(&paths.download_dir);
    let _ = std::fs::create_dir_all(&paths.logs_dir);

    let manager = BackendManager::new(cfg, paths);
    let state = AppState {
        manager: Mutex::new(manager),
        config_path: cfg_path,
        stopping: Arc::new(AtomicBool::new(false)),
    };
    app.manage(state);
    logging::info("PixivFlow Desktop started");
    Ok(())
}

/// Figures out default directories from the OS app dirs.
fn resolve_paths(app: &AppHandle) -> Paths {
    let data = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("pixivflow-desktop"));
    let resource_dir = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into());
    Paths {
        data_dir: data.join("data"),
        download_dir: data.join("downloads"),
        logs_dir: data.join("logs"),
        resource_dir,
    }
}

fn resolve_non_empty(s: &str, fallback: PathBuf) -> PathBuf {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        PathBuf::from(t)
    }
}

/// Boots the app: decides target URL and creates the main window.
fn boot(app: &AppHandle) {
    let (mode, auto_start, remote_url) = {
        let st = app.state::<AppState>();
        let mgr = st.manager.lock().unwrap();
        (mgr.cfg.mode.clone(), mgr.cfg.backend.auto_start, mgr.cfg.remote_url.clone())
    };

    // Show the boot splash right away so users see *something* while the
    // backend warms up (startup can take a few seconds).
    let _ = open_shell(app, "PixivFlow Desktop");

    if mode == "remote" {
        if remote_url.trim().is_empty() {
            let _ = show_error(app, "config.mode is \"remote\" but remoteUrl is empty");
        } else {
            open_main(app, &remote_url, "PixivFlow Desktop (remote)");
            let _ = close_boot(app);
        }
        return;
    }

    if !auto_start {
        logging::warn("backend.autoStart is false — backend not started");
        return;
    }

    // Start the backend and wait for health.
    let started = {
        let st = app.state::<AppState>();
        let mut mgr = st.manager.lock().unwrap();
        match mgr.start() {
            Ok(_) => mgr.wait_healthy().is_ok(),
            Err(e) => {
                logging::error(&e);
                false
            }
        }
    };

    if started {
        let url = {
            let st = app.state::<AppState>();
            let mgr = st.manager.lock().unwrap();
            mgr.backend_url()
        };
        open_main(app, &url, "PixivFlow Desktop");
        let _ = close_boot(app);
        logging::info(&format!("backend healthy at {url}"));
    } else {
        let msg = {
            let st = app.state::<AppState>();
            let mgr = st.manager.lock().unwrap();
            mgr.last_error.clone().unwrap_or_else(|| "backend failed to start".to_string())
        };
        logging::error(&msg);
        let _ = show_error(app, &msg);
    }
}

/// Replaces the boot splash with the real WebUI window once we know startup
/// succeeded. Uses `destroy()` (not `close()`) so the window is dropped without
/// firing our CloseRequested handler, which would otherwise quit the app.
fn close_boot(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(BOOT_WINDOW) {
        w.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_main(app: &AppHandle, url: &str, title: &str) -> Result<(), String> {
    let url = Url::parse(url).map_err(|e| format!("invalid URL {url}: {e}"))?;
    let app = app.clone();
    app.run_on_main_thread(move |h| {
        let _ = create_window(h, MAIN_WINDOW, WebviewUrl::External(url), title, 1240.0, 820.0, true);
    })
    .map_err(|e| e.to_string())
}

fn open_shell(app: &AppHandle, title: &str) -> Result<(), String> {
    let app = app.clone();
    app.run_on_main_thread(move |h| {
        let _ = create_window(h, BOOT_WINDOW, WebviewUrl::App("index.html".into()), title, 1240.0, 820.0, true);
    })
    .map_err(|e| e.to_string())
}

fn show_error(app: &AppHandle, msg: &str) -> Result<(), String> {
    let _ = app.state::<AppState>(); // ensure state exists
    logging::error(msg);
    // If the boot splash is already up, surface the error via its title
    // instead of opening a second window.
    if let Some(w) = app.get_webview_window(BOOT_WINDOW) {
        let title = format!("PixivFlow Desktop — 启动失败: {msg}");
        return w.set_title(&title).map_err(|e| e.to_string());
    }
    open_shell(app, "PixivFlow Desktop — 启动失败")
}

fn create_window(
    app: &AppHandle,
    label: &str,
    url: WebviewUrl,
    title: &str,
    w: f64,
    h: f64,
    resizable: bool,
) -> Result<(), tauri::Error> {
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.set_focus();
        return Ok(());
    }
    let wnd = WebviewWindowBuilder::new(app, label, url)
        .title(title)
        .inner_size(w, h)
        .resizable(resizable)
        .build()?;
    wnd.show()
}

fn focus_any_window(app: &AppHandle) -> Result<(), String> {
    for label in [MAIN_WINDOW, BOOT_WINDOW] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_focus();
        }
    }
    Ok(())
}

/// Gracefully stops the backend, guarding against duplicate calls.
pub fn stop_backend(app: &AppHandle) {
    let st = app.state::<AppState>();
    if st.stopping.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let mut mgr = st.manager.lock().unwrap();
    let _ = mgr.stop();
    logging::info("backend stopped on app exit");
}