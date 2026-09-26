//! Headless integration test for the F1 BackendManager contract:
//! start -> health-check -> graceful stop (the "spawn/health/close" chain).

use pixivflow_desktop::backend::manager::BackendManager;
use pixivflow_desktop::config::{AppConfig, BackendConfig, RemoteConfig};
use std::time::Duration;

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

#[test]
fn manager_start_health_stop_roundtrip() {
    let port: u16 = 3317;
    let mut m = BackendManager::new(config(port));

    assert!(!m.is_running(), "fresh manager must not be running");
    assert_eq!(m.pid(), None);

    // start
    let pid = m.start().expect("start() should spawn the backend");
    assert!(m.is_running(), "start() should mark the manager running");
    assert!(pid > 0);

    // start must be idempotent -> same pid, no duplicate process
    assert_eq!(m.start().unwrap(), pid, "start() twice must return the same pid");

    // health: poll until the mock answers 200 (bounded to ~5s)
    let mut became_healthy = false;
    for _ in 0..25 {
        if m.health_check().unwrap_or(false) {
            became_healthy = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(became_healthy, "backend on port {port} should report /api/health 200");

    // graceful stop (SIGTERM + wait) -> process reaped, no longer healthy
    m.stop().expect("stop() should terminate gracefully");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!m.is_running(), "after stop() the backend must not be running");
    assert_eq!(m.health_check().unwrap(), false, "after stop() health must read false");

    // stopping again is a no-op (already stopped)
    m.stop().expect("double stop() must be a no-op");
}

// ---- F2.1: Real Backend Adapter -----------------------------------------

use pixivflow_desktop::backend::discovery::{discover, discover_with_version, probe_version};

fn mock_path() -> String {
    String::from(env!("CARGO_MANIFEST_DIR")) + "/resources/mock-backend.mjs"
}

fn config_with_command(port: u16, command: &str, args: Vec<String>) -> AppConfig {
    let mut c = config(port);
    c.backend.command = command.into();
    c.backend.args = args;
    c
}

#[test]
fn adapter_parses_config_command() {
    // config points the adapter at a real/arbitrary backend command
    let c = config_with_command(3101, "/opt/pixivflow/bin/pixivflow".into(), vec!["--no-cron".into()]);
    let d = discover(&c);
    assert_eq!(d.source, pixivflow_desktop::backend::BackendSource::Config);
    assert!(d.is_real(), "configured command is a real backend");
    assert_eq!(d.executable_path.as_deref(), Some("/opt/pixivflow/bin/pixivflow"));
    // argv = [exe, ...args]
    assert_eq!(d.command, vec!["/opt/pixivflow/bin/pixivflow", "--no-cron"]);
}

#[test]
fn discovery_falls_back_to_nonempty_command() {
    // empty config must still resolve SOME command (mock fallback or a PATH hit)
    let d = discover(&config(3102));
    assert!(
        !d.command.is_empty(),
        "empty backend config must resolve to mock fallback or a PATH `pixivflow`"
    );
}

#[test]
fn probe_version_reports_mock_version() {
    // the mock backend answers `--version` like a real backend would
    let v = probe_version(&["node".into(), mock_path()]);
    assert!(v.is_some(), "--version probe should yield a string");
    let s = v.unwrap();
    assert!(s.contains("pixivflow"), "mock version should mention pixivflow, got {s:?}");
}

#[test]
fn fake_backend_start_health_stop_via_adapter() {
    // drive a (mock-as-fake) backend through the adapter-injected command,
    // proving the lifecycle path that a REAL backend uses.
    let port: u16 = 3103;
    let mut m = BackendManager::new(config(port));
    m.set_command_override(Some(("node".into(), vec![mock_path()])));

    let _pid = m.start().expect("adapter command should spawn the fake backend");
    assert!(m.is_running());

    let mut healthy = false;
    for _ in 0..25 {
        if m.health_check().unwrap_or(false) {
            healthy = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(healthy, "fake backend on port {port} should answer /api/health 200");

    m.stop().expect("graceful stop should work through the adapter command");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!m.is_running(), "adapter-driven backend must stop cleanly");
}

#[test]
fn doctor_report_components_resolve_for_configured_backend() {
    let c = config_with_command(3104, "node".into(), vec![mock_path()]);
    let d = discover_with_version(&c);
    assert!(d.is_real());
    assert!(d.version.is_some(), "doctor needs a version for a configured backend");
}
