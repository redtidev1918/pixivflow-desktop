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
            command: Default::default(),
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
