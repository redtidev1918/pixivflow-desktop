//! PixivFlow Desktop — Tauri shell library crate.

pub mod backend;
mod commands;
pub mod config;
pub mod i18n;
pub mod logger;
pub mod login_window;
mod link;
mod notify;
mod reveal;

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;

use backend::manager::BackendManager;
use config::AppConfig;
use logger::{LastRun, Logger, SessionInfo};
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::Manager;

/// Shared application state, managed by Tauri so commands can reach it.
pub struct ManagedState {
    pub manager: Mutex<BackendManager>,
    pub config_path: String,
    pub config_error: Mutex<Option<String>>,
    pub log: Logger,
    /// Cancel handle of a sign-in that is currently on screen. The host owns the
    /// wait (the WebUI only ever sees a promise), and the embedded view has no
    /// window chrome to close, so the menu bar's "取消登录" — and nothing else the
    /// remote page can reach — ends it from Rust.
    pub login_cancel: Mutex<Option<mpsc::Sender<()>>>,
}

/// Preferred log directory: `logDir` from the config when set, else the OS log
/// directory (`~/Library/Logs/<bundle-id>/` on macOS), else the CWD-relative
/// `logs/` the app used to scatter logs from.
fn resolve_log_dir(app: &tauri::AppHandle, cfg: &AppConfig) -> PathBuf {
    if !cfg.log_dir.trim().is_empty() {
        return PathBuf::from(cfg.log_dir.trim_end_matches('/'));
    }
    match app.path().app_log_dir() {
        Ok(dir) => dir,
        Err(_) => PathBuf::from("logs"),
    }
}

/// Open the logger in [`resolve_log_dir`], falling back to a CWD-relative
/// `logs/` when that location is unusable (read-only home, a bad `logDir`).
///
/// Returns the directory the logger really landed in, so `last-run.json` and the
/// collected crash reports sit next to the log they describe.
fn open_logger(app: &tauri::AppHandle, cfg: &AppConfig) -> (Logger, PathBuf) {
    let preferred = resolve_log_dir(app, cfg);
    if std::fs::create_dir_all(&preferred).is_ok() {
        let path = preferred.join("desktop.log").display().to_string();
        if let Ok(logger) = Logger::open_rotating(&path) {
            return (logger, preferred);
        }
    }
    let fallback = PathBuf::from("logs");
    let _ = std::fs::create_dir_all(&fallback);
    let path = fallback.join("desktop.log").display().to_string();
    (Logger::new(&path).expect("logger init"), fallback)
}

/// Trace the run events that can explain an exit. Window events keep the window
/// label — a report must be able to say *which* window closed — while the
/// per-click events (`logger::NOISY_WINDOW_EVENTS`) and the per-iteration
/// bookkeeping events (`logger::is_logged_run_event`) are skipped, so the log
/// stays small enough to read and to keep its history.
fn log_run_event(log: &Logger, event: &tauri::RunEvent) {
    let debug = format!("{event:?}");
    match event {
        tauri::RunEvent::WindowEvent { label, event, .. } => {
            let inner = format!("{event:?}");
            if logger::is_noisy_window_event(&inner) {
                return;
            }
            log.info(&format!("run event: window[{label}] {inner}"));
        }
        _ if !logger::is_logged_run_event(&debug) => {}
        other => log.info(&format!("run event: {other:?}")),
    }
}

/// Flip the current run's `last-run.json` to a clean exit. Best effort.
fn mark_clean_exit(log: &Logger) {
    let Some(dir) = logger::panic_log_path().and_then(|p| p.parent()).map(PathBuf::from) else {
        return;
    };
    let Some(mut run) = logger::read_last_run(&dir) else { return };
    run.clean_exit = true;
    run.exited_at = Some(logger::unix_now());
    if let Err(e) = logger::write_last_run(&dir, &run) {
        log.warn(&format!("cannot update last-run.json: {e}"));
    }
}

/// Menu ids [`build_menu`] installs and the handler in [`run`] reacts to.
const MENU_MANAGER: &str = "open-manager";
const MENU_LOGS: &str = "open-logs";
const MENU_DIAGNOSTICS: &str = "collect-diagnostics";
const MENU_CANCEL_LOGIN: &str = "cancel-login";

/// The app menu is where the host's own controls live now that the manager
/// window is no longer the app's front door. Everything the launcher page used to
/// offer (the manager itself as a fallback view, the log file, the diagnostics
/// export) has to stay reachable without it — and "取消登录" is only reachable
/// here: while the Pixiv sign-in covers the WebUI window, the remote page holds
/// the keyboard and there is no window chrome to close.
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let manager_item =
        MenuItemBuilder::with_id(MENU_MANAGER, i18n::t_static("打开管理器", "Open manager"))
            .build(app)?;
    let logs_item =
        MenuItemBuilder::with_id(MENU_LOGS, i18n::t_static("打开日志", "Open logs")).build(app)?;
    let diagnostics_item = MenuItemBuilder::with_id(
        MENU_DIAGNOSTICS,
        i18n::t_static("收集诊断", "Collect diagnostics"),
    )
    .build(app)?;
    let cancel_item = MenuItemBuilder::with_id(
        MENU_CANCEL_LOGIN,
        i18n::t_static("取消登录", "Cancel sign-in"),
    )
    .build(app)?;

    // The macOS-only predefined items are cfg'd out on other platforms, where
    // `about`/`services`/`hide`/`hide_others`/`show_all` do not exist.
    #[cfg(target_os = "macos")]
    let app_menu = SubmenuBuilder::new(app, "PixivFlow Desktop")
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    #[cfg(not(target_os = "macos"))]
    let app_menu = SubmenuBuilder::new(app, "PixivFlow Desktop").quit().build()?;

    let host_menu = SubmenuBuilder::new(app, "PixivFlow")
        .item(&manager_item)
        .item(&logs_item)
        .item(&diagnostics_item)
        .separator()
        .item(&cancel_item)
        .build()?;
    let edit_menu = SubmenuBuilder::new(app, i18n::t_static("编辑", "Edit"))
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window_menu = SubmenuBuilder::new(app, i18n::t_static("窗口", "Window"))
        .minimize()
        .close_window()
        .build()?;

    MenuBuilder::new(app)
        .items(&[&app_menu, &host_menu, &edit_menu, &window_menu])
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch is meant to bring the app forward: the WebUI is the
            // app, the manager only exists as the fallback view.
            let target = app
                .get_webview_window("webui")
                .or_else(|| app.get_webview_window("main"));
            if let Some(w) = target {
                let _ = w.show();
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
            commands::open_webui,
            commands::open_login_window,
            commands::reveal_path,
            commands::notify,
            commands::open_external,
            commands::open_in_app,
            commands::export_diagnostics,
            commands::log_frontend,
            i18n::get_locale,
        ])
        .on_menu_event(|handle, event| match event.id().as_ref() {
            MENU_MANAGER => {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            MENU_LOGS => {
                let _ = commands::open_logs(handle.state::<ManagedState>());
            }
            MENU_DIAGNOSTICS => {
                let _ = commands::export_diagnostics(handle.clone(), handle.state::<ManagedState>());
            }
            MENU_CANCEL_LOGIN => {
                commands::cancel_login(handle);
            }
            _ => {}
        })
        .setup(|app| {
            // Panic hook first: everything below — and every later thread —
            // must leave a durable record before any window/backend work.
            logger::install_panic_hook();

            // 0. point discovery at the installed bundle resources (no-op in dev,
            //    where it falls back to the cargo manifest dir) and give the real
            //    backend a user-writable data root so it never writes into CWD.
            if let Ok(res_dir) = app.path().resource_dir() {
                backend::discovery::set_resource_root(res_dir.clone());
            }
            if let Ok(data_dir) = app.path().app_local_data_dir() {
                let data_dir = data_dir.join("pixivflow");
                let _ = std::fs::create_dir_all(&data_dir);
                backend::discovery::set_data_root(data_dir);
            }

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

            // 2. logs — the OS log directory by default, so a launched app stops
            //    scattering `logs/` across the repo, `src-tauri/` and `/tmp`.
            //    `logDir` in the config still wins and `logs/` stays the fallback.
            let (logger, log_dir) = open_logger(app.handle(), &cfg);
            logger::set_panic_log_path(PathBuf::from(logger.path()));

            let state = ManagedState {
                manager: Mutex::new(BackendManager::new(cfg.clone())),
                config_path: config_path.display().to_string(),
                config_error: Mutex::new(config_error),
                log: logger,
                login_cancel: Mutex::new(None),
            };
            state.log.info(&format!(
                "==== PixivFlow Desktop F1 started ==== (mode={} port={})",
                cfg.mode,
                cfg.port()
            ));

            let version = app.package_info().version.to_string();
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map(|d| d.join("pixivflow").display().to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            let resource_dir = app
                .path()
                .resource_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            // Mirrors what `BackendManager::start()` will decide: the login token
            // exchange and every download run inside the backend process, which a
            // Finder/Dock launch starts with no inherited environment.
            // Availability only: the injection decision needs the resolved
            // LaunchSpec and is logged by `backend proxy: ...` once it is made.
            let proxy_available = backend::proxy::system_proxy().is_some();
            state.log.session_marker(&SessionInfo {
                version: &version,
                log_path: state.log.path(),
                config_path: &state.config_path,
                data_dir: &data_dir,
                resource_dir: &resource_dir,
                locale: i18n::locale(),
                proxy_available,
            });
            if let Some(cfg_err) = &*state.config_error.lock().unwrap() {
                state.log.error(&format!("{cfg_err} (使用默认配置)"));
            }

            // 3. bookkeeping: warn about a run that never exited cleanly and keep
            //    the native crash evidence it left behind, then record this run.
            let started_at = logger::unix_now();
            logger::check_previous_run(&state.log, &log_dir, started_at);
            let current_run = LastRun {
                started_at,
                clean_exit: false,
                pid: std::process::id(),
                version: version.clone(),
                exited_at: None,
            };
            if let Err(e) = logger::write_last_run(&log_dir, &current_run) {
                state.log.warn(&format!("cannot write last-run.json: {e}"));
            }

            app.manage(state);

            // 3b. the app menu: the host's controls, including the only way out of
            //     an embedded sign-in view (see [`build_menu`]).
            match build_menu(app.handle()) {
                Ok(menu) => {
                    if let Err(e) = app.set_menu(menu) {
                        eprintln!("menu install failed: {e}");
                    }
                }
                Err(e) => eprintln!("menu build failed: {e}"),
            }

            // 4. auto-start the local backend in the background
            let app_handle = app.handle().clone();
            let auto_port = cfg.port();
            if cfg.is_local() && cfg.backend.auto_start {
                std::thread::spawn(move || {
                    {
                        let state = app_handle.state::<ManagedState>();
                        commands::apply_discovery(&state);
                        let start_res = state.manager.lock().unwrap().start();
                        match start_res {
                            Ok(pid) => {
                                let adopted = state.manager.lock().unwrap().is_adopted();
                                let what = if adopted { "adopted" } else { "started" };
                                state.log.info(&format!("auto-start: backend {what} pid={pid}"));
                                if !adopted {
                                    state.log.info(&format!(
                                        "backend proxy: {}",
                                        state.manager.lock().unwrap().last_proxy_injection().describe()
                                    ));
                                }
                            }
                            Err(e) => {
                                state.log.error(&format!("auto-start failed: {e}"))
                            }
                        }
                    }
                    // 5. once the backend answers healthy, hand the user straight
                    //    to the WebUI — no manual click on "打开 PixivFlow".
                    //    Window creation must happen on the main thread.
                    if commands::poll_health_for(&app_handle) {
                        let opener = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            match commands::open_webui_window(&opener, auto_port) {
                                Ok(url) => {
                                    commands::log_scale_factors(&opener);
                                    if let Some(state) = opener.try_state::<ManagedState>() {
                                        state.log.info(&format!("auto-open: WebUI at {url}"));
                                    }
                                }
                                Err(e) => {
                                    if let Some(state) = opener.try_state::<ManagedState>() {
                                        state.log.warn(&format!("auto-open WebUI failed: {e}"));
                                    }
                                }
                            }
                        });
                    } else {
                        // Nothing was opened, so the hidden manager window is the
                        // only place that can say what went wrong: show it.
                        if let Some(state) = app_handle.try_state::<ManagedState>() {
                            state.log.warn(
                                "backend did not become healthy — showing the manager window",
                            );
                        }
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                });
            }

            Ok(())
        })

        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // The WebUI window IS the app: closing it quits, taking the
                // backend (and the port it holds) with it. The manager window is
                // only the fallback view, so closing it merely hides it while a
                // WebUI window is still up — and quits when there is none.
                let label = window.label().to_string();
                let h = window.app_handle().clone();
                if label == "main" && h.get_webview_window("webui").is_some() {
                    return;
                }
                std::thread::spawn(move || {
                    if let Some(webui) = h.get_webview_window("webui") {
                        let _ = webui.close();
                    }
                    if let Some(state) = h.try_state::<ManagedState>() {
                        match state.manager.lock().unwrap().stop() {
                            Ok(()) => state.log.info("window closed: backend stopped (graceful)"),
                            Err(e) => state.log.warn(&format!("window closed: stop backend failed: {e}")),
                        }
                    }
                    // Closing the last window has to end the process: no window
                    // is left to trigger the exit path otherwise.
                    h.exit(0);
                });
            }
        })

        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Quitting the app (Cmd+Q, the app menu, an AppleScript `quit`, a logout)
    // never reaches the window close handler above, so the backend would be left
    // orphaned holding the port. Stop it here as well: `stop()` is idempotent and
    // adoption covers a backend that still survives an abrupt kill.
    app.run(|handle, event| {
        let state = handle.try_state::<ManagedState>();

        // 5b. an embedded sign-in view is positioned by us and does not follow its
        //     parent on its own, so it is resized whenever the WebUI window is.
        if let tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Resized(size),
            ..
        } = &event
        {
            if label == "webui" {
                commands::resize_login_overlay(handle, *size);
            }
        }

        // 6. run-event trace (high-frequency focus/scale events are filtered out).
        if let Some(state) = &state {
            log_run_event(&state.log, &event);
        }

        // Both variants are handled: `ExitRequested` covers the last window being
        // destroyed / an explicit `exit()`, `Exit` covers a macOS quit (Cmd+Q,
        // app menu, AppleScript `quit`), which tears the app down without ever
        // reaching the window close handler.
        let exiting = matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        );
        if let (true, Some(state)) = (exiting, &state) {
            match state.manager.lock().unwrap().stop() {
                Ok(()) => state.log.info(&format!("app exit ({event:?}): backend stopped (graceful)")),
                Err(e) => state.log.warn(&format!("app exit: stop backend failed: {e}")),
            }
        }
        // 7. a clean quit is what tells the next startup that the previous run
        //    did *not* crash — recorded last, after the backend is down.
        if let (true, Some(state)) = (exiting, &state) {
            mark_clean_exit(&state.log);
        }
    });
}
