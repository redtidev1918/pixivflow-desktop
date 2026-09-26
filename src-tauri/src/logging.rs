//! Tiny append-only file logger for the desktop app's own diagnostics.
//! Writes to `<logs_dir>/pixivflow-desktop.log`. Keeps the dependency surface
//! to zero (std only). Not thread-racing: guarded by a global Mutex<Option<File>>.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

static LOG: Mutex<Option<File>> = Mutex::new(None);

/// Initializes the logger (no-op if already initialized or logs_dir is empty).
pub fn init(logs_dir: &Path) {
    if let Ok(mut guard) = LOG.lock() {
        if guard.is_some() {
            return;
        }
        let _ = std::fs::create_dir_all(logs_dir);
        let path = logs_dir.join("pixivflow-desktop.log");
        if let Ok(f) = OpenOptions::new().create(true).append(true).open(path) {
            *guard = Some(f);
        }
    }
}

/// Appends a line like `2026-01-01T12:00:00.123Z [INFO] message` to the log file
/// and, when `stderr: true`, to stderr (useful during `tauri dev`).
pub fn log(level: &str, msg: &str) {
    let ts = timestamp();
    let line = format!("{ts} [{level}] {msg}\n");
    if let Ok(mut guard) = LOG.lock() {
        if let Some(f) = guard.as_mut() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
    eprintln!("{line}");
}

pub fn info(msg: &str) {
    log("INFO", msg);
}
pub fn error(msg: &str) {
    log("ERROR", msg);
}
pub fn warn(msg: &str) {
    log("WARN", msg);
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let millis = now.as_millis();
    let secs = millis / 1000;
    // UTC wall-clock formatting without chrono.
    let days = secs as i64 / 86400;
    let rem = secs as i64 % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let ms = millis % 1000;
    // Rough civil date from days since epoch (Howard Hinnant algorithm).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yy = if mo <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        yy, mo, d, h, m, s, ms
    )
}