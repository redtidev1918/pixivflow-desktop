//! Headless integration tests for the BackendManager contract:
//! start -> health-check -> graceful stop, PLUS the F2.1/F2.2 adapter behavior
//! (discovery precedence, runtime-manifest parsing, 方案 A WebUI static hosting).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use pixivflow_desktop::backend::manager::{BackendManager, LaunchSpec};
use pixivflow_desktop::backend::discovery::{discover, discover_with_version, probe_version, static_webui_path};
use pixivflow_desktop::backend::BackendSource;
use pixivflow_desktop::config::{AppConfig, BackendConfig, RemoteConfig};

fn config(port: u16) -> AppConfig {
    AppConfig {
        mode: "local".into(),
        backend: BackendConfig {
            port,
            auto_start: true,
            command: String::new(),
            args: Vec::new(),
        },
        data_dir: "".into(),
        download_dir: "".into(),
        log_dir: "".into(),
        remote: RemoteConfig::default(),
    }
}

fn config_with_command(port: u16, command: &str, args: Vec<String>) -> AppConfig {
    let mut c = config(port);
    c.backend.command = command.into();
    c.backend.args = args;
    c
}

fn mock_path() -> String {
    String::from(env!("CARGO_MANIFEST_DIR")) + "/resources/mock-backend.mjs"
}

/// The committed lightweight dev stand-in, independent of whichever runtime the
/// FETCHED manifest points at. Tests that need a known-benign backend use this
/// directly so they pass whether the real runtime has been laid out or not.
fn dev_standin_path() -> String {
    String::from(env!("CARGO_MANIFEST_DIR")) + "/resources/runtime/pixivflow/dev-backend.mjs"
}

fn wait_healthy(m: &BackendManager) -> bool {
    for _ in 0..25 {
        if m.health_check().unwrap_or(false) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn http_get_body(port: u16, path: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut b = String::new();
    s.read_to_string(&mut b).unwrap();
    b
}

// ---- F1: core lifecycle ------------------------------------------------

#[test]
fn manager_start_health_stop_roundtrip() {
    let port: u16 = 3317;
    let mut m = BackendManager::new(config(port));

    assert!(!m.is_running(), "fresh manager must not be running");
    assert_eq!(m.pid(), None);

    let pid = m.start().expect("start() should spawn the backend");
    assert!(m.is_running(), "start() should mark the manager running");
    assert!(pid > 0);
    assert_eq!(m.start().unwrap(), pid, "start() twice must return the same pid");

    assert!(wait_healthy(&m), "backend on port {port} should report /api/health 200");

    m.stop().expect("stop() should terminate gracefully");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!m.is_running(), "after stop() the backend must not be running");
    assert_eq!(m.health_check().unwrap(), false, "after stop() health must read false");
    m.stop().expect("double stop() must be a no-op");
}

// ---- F2.1: Real Backend Adapter -----------------------------------------

#[test]
fn fake_backend_start_health_stop_via_adapter() {
    // Drive a (mock-as-fake) backend through an adapter-injected LaunchSpec,
    // proving the lifecycle path a REAL backend uses.
    let port: u16 = 3103;
    let mut m = BackendManager::new(config(port));
    m.set_command_override(Some(LaunchSpec {
        command: vec!["node".into(), mock_path()],
        env: Default::default(),
    }));

    m.start().expect("adapter command should spawn the fake backend");
    assert!(m.is_running());
    assert!(wait_healthy(&m), "fake backend on port {port} should answer /api/health 200");

    m.stop().expect("graceful stop should work through the adapter command");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!m.is_running(), "adapter-driven backend must stop cleanly");
}

#[test]
fn config_command_launches_when_no_adapter_override() {
    // BackendManager resolves config.backend.command when discovery hasn't set
    // an override (user "direct" path, independent of bundled/PATH precedence).
    let port: u16 = 3106;
    let mut m = BackendManager::new(config_with_command(port, "node", vec![mock_path()]));
    let pid = m.start().unwrap();
    assert!(pid > 0);
    assert!(m.is_running());
    assert!(wait_healthy(&m), "config command backend should be healthy");
    m.stop().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(!m.is_running());
}

// ---- F2.2: Bundled runtime (manifest) + precedence + 方案 A WebUI ----------

#[test]
fn bundled_takes_precedence_over_config_and_path() {
    // F2.2.2: even when config names a command (or PATH carries another
    // `pixivflow`), the bundled runtime must win so a stray binary can't
    // perturb a shipped install.
    let c = config_with_command(3105, "/opt/fake/pixivflow".into(), Vec::new());
    let d = discover(&c);
    assert_eq!(d.source, BackendSource::Bundled, "bundled must shadow config");
    assert!(d.is_real());
    assert!(d.serves_webui, "bundled runtime declares 方案 A WebUI hosting");
    // Whatever the manifest target is (dev stand-in or fetched real runtime),
    // the executable must resolve inside the bundled runtime dir.
    let exe = d.executable_path.as_deref().expect("bundled exe path");
    assert!(
        exe.contains("runtime/pixivflow"),
        "bundled exe must live in the runtime dir, got {exe:?}"
    );
}

#[test]
fn bundled_manifest_provides_static_webui_path() {
    let d = discover(&config(3108));
    assert_eq!(d.source, BackendSource::Bundled);
    let sp = d.static_path.as_deref().expect("static_path for bundled webui");
    assert!(sp.contains("webui/dist"), "STATIC_PATH should point at webui/dist, got {sp:?}");
    // the runtime manifest carries the version (no `--version` probe needed).
    assert!(d.version.is_some(), "manifest must version the bundled runtime");
}

#[test]
fn bundled_runtime_serves_webui_static_and_stops() {
    // End-to-end 方案 A: a manifest-contract backend serves the bundled WebUI
    // dist over STATIC_PATH and answers health; it must stop gracefully. We
    // drive the committed dev stand-in explicitly so this stays fast and
    // deterministic whether or not the real runtime has been fetched.
    let port: u16 = 3110;
    let mut m = BackendManager::new(config(port));
    let mut env = std::collections::BTreeMap::new();
    env.insert(
        "STATIC_PATH".to_string(),
        static_webui_path().expect("bundled webui dist present"),
    );
    m.set_command_override(Some(LaunchSpec {
        command: vec!["node".into(), dev_standin_path()],
        env,
    }));
    m.start().unwrap();
    assert!(wait_healthy(&m), "bundled backend should be healthy on {port}");

    // 方案 A: GET / serves the bundled webui index (not the control UI).
    let body = http_get_body(port, "/");
    assert!(body.starts_with("HTTP/1.1 200"), "GET / should be 200, got {body:?}");
    assert!(body.contains("PixivFlow WebUI"), "static webui index must be served");

    m.stop().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(!m.is_running(), "bundled backend must stop via SIGTERM");
}

// -- discovery fallback / doctor ------------------------------------------

#[test]
fn discovery_falls_back_to_nonempty_command() {
    let d = discover(&config(3102));
    assert!(!d.command.is_empty(), "must resolve bundled/mock/path to a command");
}

#[test]
fn probe_version_reports_mock_version() {
    let v = probe_version(&["node".into(), mock_path()]);
    assert!(v.is_some(), "--version probe should yield a string");
    assert!(v.unwrap().contains("pixivflow"));
}

#[test]
fn doctor_resolves_bundled_runtime_and_version() {
    let d = discover_with_version(&config(3104));
    assert_eq!(d.source, BackendSource::Bundled);
    assert!(d.is_real());
    // version comes from the runtime manifest (trusted, no `--version` probe on
    // a real release binary). Assert it surfaces SOME version.
    assert!(d.version.is_some(), "doctor needs a version from the manifest");
}
