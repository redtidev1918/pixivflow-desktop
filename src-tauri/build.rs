//! Tauri build script.
//!
//! The app ACL manifest lists every command registered in `invoke_handler!`
//! (`src-tauri/src/lib.rs`). Without it the runtime has no `allow-*` permission
//! for a custom command, so `invoke` is rejected with
//! "Command <name> not allowed by ACL". The generated permission identifiers are
//! `allow-<command>` with `_` replaced by `-`, so `open_login_window` becomes
//! `allow-open-login-window`.
fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "get_status",
                "get_config",
                "config_path",
                "start_backend",
                "stop_backend",
                "restart_backend",
                "backend_doctor",
                "open_logs",
                "open_webui",
                "open_login_window",
                "export_diagnostics",
                "log_frontend",
                "get_locale",
            ]),
        ),
    )
    .expect("failed to run tauri-build");
}
