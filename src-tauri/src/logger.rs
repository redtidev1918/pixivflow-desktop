//! Desktop logging + diagnostics (F5).
//!
//! One append-only file logger (mirrored to stdout so it is visible during
//! `cargo tauri dev`), plus everything that makes a user's log *useful* when
//! something went wrong: readable UTC timestamps, startup rotation, a
//! logger-independent panic path, previous-run bookkeeping and the macOS crash
//! report copy.
//!
//! No external logging dependency — this is deliberately small, auditable and
//! dependency-free, exactly like the rest of the system layer.
//!
//! Two rules hold everywhere in here:
//!  - **best effort**: diagnostics must never take the app down. Every failure
//!    is swallowed or reported as a WARN; nothing here aborts startup.
//!  - **never log secrets**: [`redact_secrets`] masks token/secret/password
//!    values before they reach a log line.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// Rotate `desktop.log` once it passes 2 MiB.
pub const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// Rotated generations kept: `desktop.log.1` (newest) … `desktop.log.3`.
pub const ROTATED_GENERATIONS: usize = 3;

/// One forwarded (frontend / WebUI) log message is clamped to this many
/// characters — a render loop must never be able to flood the log.
pub const MAX_FORWARDED_CHARS: usize = 2000;

/// Seconds since the Unix epoch (0 when the clock is before 1970).
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Readable, unambiguous, explicitly-UTC timestamp: `2026-09-26T18:21:48Z`.
///
/// The logger has always written UTC; the trailing `Z` is what makes that
/// obvious to a human reading a bug report from another timezone.
pub fn format_unix_ts(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let rem = unix % 86_400;
    let (y, mo, d) = civil_from_days(days);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Filename-safe UTC stamp for artifacts: `20260926T182148Z`.
pub fn compact_utc_ts(unix: u64) -> String {
    format_unix_ts(unix).replace(['-', ':'], "")
}

/// Days since the Unix epoch -> civil (year, month, day). Howard Hinnant's
/// `civil_from_days`, valid for the whole proleptic Gregorian calendar.
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

// ---------------------------------------------------------------- the logger

/// Minimal append-only file logger.
pub struct Logger {
    file: Mutex<BufWriter<File>>,
    path: String,
}

impl Logger {
    /// Open (creating parents) without rotating. Prefer [`Logger::open_rotating`]
    /// for the real startup path.
    pub fn new(path: &str) -> std::io::Result<Self> {
        let p = PathBuf::from(path);
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = OpenOptions::new().create(true).append(true).open(&p)?;
        Ok(Self { file: Mutex::new(BufWriter::new(file)), path: path.to_string() })
    }

    /// Rotate a too-large log first (2 MiB, 3 generations), then open fresh.
    pub fn open_rotating(path: &str) -> std::io::Result<Self> {
        let _ = rotate_log(Path::new(path), MAX_LOG_BYTES, ROTATED_GENERATIONS);
        Self::new(path)
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

    /// One startup banner with everything a maintainer needs to reproduce a
    /// report: build, machine, paths, locale and whether a proxy was injected.
    pub fn session_marker(&self, info: &SessionInfo<'_>) {
        self.info(&format!(
            "session: version={} os={} arch={} pid={}",
            info.version,
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::process::id()
        ));
        self.info(&format!("session: log={}", info.log_path));
        self.info(&format!("session: config={}", info.config_path));
        self.info(&format!("session: data dir={}", info.data_dir));
        self.info(&format!("session: resource dir={}", info.resource_dir));
        self.info(&format!("session: locale={}", info.locale));
        self.info(&format!(
            "session: system proxy available={}",
            info.proxy_available
        ));
    }
}

/// Everything the startup session marker reports.
#[derive(Debug, Clone, Default)]
pub struct SessionInfo<'a> {
    pub version: &'a str,
    pub log_path: &'a str,
    pub config_path: &'a str,
    pub data_dir: &'a str,
    pub resource_dir: &'a str,
    pub locale: &'a str,
    /// Whether the OS reports a system proxy at all. The decision to *inject*
    /// it is made when the backend starts and is logged by
    /// `backend proxy: ...` — this marker only reports availability.
    pub proxy_available: bool,
}

// -------------------------------------------------------------- log rotation

/// `desktop.log` + generation -> `desktop.log.N`.
pub fn rotated_path(log_path: &Path, generation: usize) -> PathBuf {
    let mut name = log_path.as_os_str().to_os_string();
    name.push(format!(".{generation}"));
    PathBuf::from(name)
}

/// Startup rotation. When `log_path` is larger than `max_bytes`, shift
/// `.2`->`.3`, `.1`->`.2`, `log`->`.1` (keeping `keep` generations) so the next
/// open starts a fresh file. Returns whether a rotation happened.
///
/// Best effort: a missing file is `Ok(false)` and an individual failed rename is
/// ignored — a broken rotation must never stop the app from logging.
pub fn rotate_log(log_path: &Path, max_bytes: u64, keep: usize) -> std::io::Result<bool> {
    let len = match std::fs::metadata(log_path) {
        Ok(m) if m.is_file() => m.len(),
        _ => return Ok(false),
    };
    if len <= max_bytes {
        return Ok(false);
    }
    for generation in (1..keep).rev() {
        let from = rotated_path(log_path, generation);
        if from.exists() {
            let to = rotated_path(log_path, generation + 1);
            let _ = std::fs::remove_file(&to);
            let _ = std::fs::rename(&from, &to);
        }
    }
    let _ = std::fs::remove_file(rotated_path(log_path, 1));
    std::fs::rename(log_path, rotated_path(log_path, 1))?;
    Ok(true)
}

// --------------------------------------------------------------- panic path

/// The live log file, published once at startup so the panic hook — which may
/// run while the `Logger` mutex is held — can append without it.
static PANIC_LOG: OnceLock<PathBuf> = OnceLock::new();

/// Publish the log path for [`append_panic`]. Idempotent; later calls are no-ops.
pub fn set_panic_log_path(path: PathBuf) {
    let _ = PANIC_LOG.set(path);
}

pub fn panic_log_path() -> Option<&'static Path> {
    PANIC_LOG.get().map(PathBuf::as_path)
}

/// `logs/desktop.log` -> `logs/panic-20260926T182148Z.log`.
pub fn panic_sibling_path(log_path: &Path, unix: u64) -> PathBuf {
    log_path.with_file_name(format!("panic-{}.log", compact_utc_ts(unix)))
}

/// The one-line panic record: `PANIC <payload> at <file>:<line>`.
pub fn format_panic_line(payload: &str, location: Option<(&str, u32)>) -> String {
    match location {
        Some((file, line)) => format!("PANIC {payload} at {file}:{line}"),
        None => format!("PANIC {payload} at <unknown location>"),
    }
}

/// Append raw text to `path`, creating it when needed. A FRESH handle every
/// time and no shared state, so it is safe from inside a panic.
fn append_to(path: &Path, text: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(text.as_bytes());
        let _ = f.flush();
    }
}

/// Record a panic in the live log AND in a sibling `panic-<utc-ts>.log`.
///
/// Deliberately independent of [`Logger`]: the panic may well have happened
/// while the logger mutex was held, so every lock is avoided here.
pub fn append_panic(text: &str) {
    let Some(path) = PANIC_LOG.get() else { return };
    append_to(path, text);
    append_to(&panic_sibling_path(path, unix_now()), text);
}

/// Install the process-wide panic hook.
///
/// Chains to the hook that was already installed (the std default in practice),
/// so the standard message keeps reaching stderr and `RUST_BACKTRACE` keeps
/// working; our addition is the durable file record with a forced backtrace.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Box<dyn Any>".to_string());
        let location = info.location().map(|l| (l.file(), l.line()));
        let text = format!(
            "{} {}\n{}\n",
            format_unix_ts(unix_now()),
            format_panic_line(&redact_secrets(&payload), location),
            std::backtrace::Backtrace::force_capture(),
        );
        previous(info);
        append_panic(&text);
    }));
}

// ------------------------------------------------------------- run-event trace

/// Window events that fire too often to be worth a log line: one `Focused` per
/// click and one `ScaleFactorChanged`/`Moved` per drag frame would drown out
/// everything useful.
pub const NOISY_WINDOW_EVENTS: [&str; 4] = ["Focused", "ScaleFactorChanged", "Moved", "Resized"];

/// Leading variant name of a `{:?}` rendering: `Focused(true)` -> `Focused`.
pub fn event_variant_name(debug: &str) -> &str {
    let end = debug
        .find(|c: char| c == '(' || c == '{' || c == ' ')
        .unwrap_or(debug.len());
    &debug[..end]
}

/// True when this `WindowEvent` debug rendering must not be logged.
pub fn is_noisy_window_event(debug: &str) -> bool {
    NOISY_WINDOW_EVENTS.contains(&event_variant_name(debug))
}

/// `RunEvent` variants worth a log line.
///
/// The event loop also emits bookkeeping variants: `MainEventsCleared` fires on
/// every iteration and alone produced 4 235 of 4 253 lines (~12 KB/s of log) in
/// the F4.1 verification run, which would rotate the useful history away within
/// minutes — the opposite of what a diagnostics log is for. `MenuEvent` and
/// `TrayIconEvent` carry nothing this app acts on, so they stay out too.
pub const LOGGED_RUN_EVENTS: [&str; 8] = [
    "Ready",
    "Resumed",
    "Opened",
    "Reopen",
    "WindowEvent",
    "WebviewEvent",
    "ExitRequested",
    "Exit",
];

/// True when this `RunEvent` debug rendering is worth a log line.
pub fn is_logged_run_event(debug: &str) -> bool {
    LOGGED_RUN_EVENTS.contains(&event_variant_name(debug))
}

// ------------------------------------------------------------------ redaction

/// Mask the value of any `token` / `secret` / `password` / `credential` key.
///
/// Defensive only — nothing we log today carries a secret, but text that came
/// from the WebUI or from a config file is not ours to trust. Handles the three
/// shapes those values actually take in a log line: `key=value`, `key: value`
/// and `"key": "value"`.
pub fn redact_secrets(text: &str) -> String {
    const NEEDLES: [&str; 4] = ["token", "secret", "password", "credential"];
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        let needle = NEEDLES.iter().find(|n| {
            let n: Vec<char> = n.chars().collect();
            i + n.len() <= lower.len() && lower[i..i + n.len()] == n[..]
        });
        let Some(needle) = needle else {
            out.push(chars[i]);
            i += 1;
            continue;
        };

        // Emit the key itself, then look for the assignment operator, allowing
        // at most one closing quote and whitespace in between.
        for _ in 0..needle.chars().count() {
            out.push(chars[i]);
            i += 1;
        }
        let mut j = i;
        let mut separator: Vec<char> = Vec::new();
        if j < chars.len() && (chars[j] == '"' || chars[j] == '\'') {
            separator.push(chars[j]);
            j += 1;
        }
        while j < chars.len() && chars[j].is_whitespace() {
            separator.push(chars[j]);
            j += 1;
        }
        if j >= chars.len() || (chars[j] != '=' && chars[j] != ':') {
            continue; // the needle was part of a longer word — leave it alone
        }
        for c in separator {
            out.push(c);
        }
        out.push(chars[j]);
        j += 1;
        while j < chars.len() && chars[j] == ' ' {
            out.push(' ');
            j += 1;
        }
        let quote = if j < chars.len() && (chars[j] == '"' || chars[j] == '\'') {
            let q = chars[j];
            out.push(q);
            j += 1;
            Some(q)
        } else {
            None
        };
        out.push_str("***");
        while j < chars.len() {
            let c = chars[j];
            let done = match quote {
                Some(q) => c == q,
                None => c.is_whitespace() || matches!(c, '&' | ',' | '}' | '"' | '\'' | ';'),
            };
            if done {
                break;
            }
            j += 1;
        }
        if quote.is_some() && j < chars.len() {
            out.push(chars[j]);
            j += 1;
        }
        i = j;
    }
    out
}

/// Clamp `text` to at most `max` characters (never bytes — a truncated UTF-8
/// sequence must not panic).
pub fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

// ------------------------------------------------- last-run & crash reports

/// `<log dir>/last-run.json` — enough to tell a clean quit from a crash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    pub started_at: u64,
    pub clean_exit: bool,
    pub pid: u32,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exited_at: Option<u64>,
}

pub fn last_run_path(log_dir: &Path) -> PathBuf {
    log_dir.join("last-run.json")
}

/// Write the bookkeeping file. Best effort by contract: the caller warns.
pub fn write_last_run(log_dir: &Path, run: &LastRun) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(run)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(last_run_path(log_dir), json)
}

/// Read the previous run's record; `None` when absent or unreadable.
pub fn read_last_run(log_dir: &Path) -> Option<LastRun> {
    let raw = std::fs::read_to_string(last_run_path(log_dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// `~/Library/Logs/DiagnosticReports/` on macOS, `None` elsewhere.
///
/// Rationale for the copy in [`copy_crash_report`]: macOS moves older reports
/// into `Retired/` and eventually purges them, so a copy inside the app's own
/// log directory is the only durable evidence of a native crash.
#[cfg(target_os = "macos")]
pub fn diagnostic_reports_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Logs/DiagnosticReports"))
}

#[cfg(not(target_os = "macos"))]
pub fn diagnostic_reports_dir() -> Option<PathBuf> {
    None
}

/// Newest `pixivflow-desktop-*.ips` in `dir` modified at or after
/// `prev_started_at - 60` (60 s of slack for clock skew between the two
/// timestamps). Pure directory scan — unit-testable on every platform.
pub fn newest_crash_report_in(dir: &Path, prev_started_at: u64) -> Option<PathBuf> {
    let cutoff = prev_started_at.saturating_sub(60);
    let mut newest: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("pixivflow-desktop-") || !name.ends_with(".ips") {
            continue;
        }
        let mtime = match entry.metadata().ok().and_then(|m| m.modified().ok()) {
            Some(t) => match t.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => d.as_secs(),
                Err(_) => continue,
            },
            None => continue,
        };
        if mtime < cutoff {
            continue;
        }
        if newest.as_ref().is_none_or(|(best, _)| mtime > *best) {
            newest = Some((mtime, entry.path()));
        }
    }
    newest.map(|(_, path)| path)
}

/// Copy the newest matching macOS crash report into `log_dir` as
/// `crash-<utc-ts>.ips`. `None` when there is nothing to copy (or not macOS).
pub fn copy_crash_report(log_dir: &Path, prev_started_at: u64, now: u64) -> Option<PathBuf> {
    let source = newest_crash_report_in(&diagnostic_reports_dir()?, prev_started_at)?;
    let dest = log_dir.join(format!("crash-{}.ips", compact_utc_ts(now)));
    std::fs::copy(&source, &dest).ok()?;
    Some(dest)
}

/// Startup bookkeeping: WARN about a run that did not exit cleanly and, on
/// macOS, preserve the crash report it left behind. Never aborts startup.
pub fn check_previous_run(log: &Logger, log_dir: &Path, now: u64) {
    let Some(previous) = read_last_run(log_dir) else { return };
    if previous.clean_exit {
        return;
    }
    log.warn(&format!(
        "previous run did not exit cleanly: startedAt={} ({}) pid={} version={}",
        previous.started_at,
        format_unix_ts(previous.started_at),
        previous.pid,
        previous.version
    ));
    match copy_crash_report(log_dir, previous.started_at, now) {
        Some(copied) => log.warn(&format!("collected macOS crash report -> {}", copied.display())),
        None => {
            #[cfg(target_os = "macos")]
            log.info("no new pixivflow-desktop-*.ips report found for the previous run");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "pixivflow-desktop-test-{}-{}-{n}-{label}",
            std::process::id(),
            unix_now()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ---- timestamps -----------------------------------------------------

    #[test]
    fn formats_known_utc_timestamps() {
        assert_eq!(format_unix_ts(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_ts(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(format_unix_ts(1_790_446_908), "2026-09-26T18:21:48Z");
    }

    #[test]
    fn formats_the_leap_day() {
        assert_eq!(format_unix_ts(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format_unix_ts(1_709_168_460), "2024-02-29T01:01:00Z");
        // the day after a leap day, and the last second of a year
        assert_eq!(format_unix_ts(1_709_251_200), "2024-03-01T00:00:00Z");
        assert_eq!(format_unix_ts(1_735_689_599), "2024-12-31T23:59:59Z");
    }

    #[test]
    fn compact_stamp_is_filename_safe() {
        assert_eq!(compact_utc_ts(1_790_446_908), "20260926T182148Z");
        assert!(!compact_utc_ts(1_790_446_908).contains(':'));
    }

    // ---- rotation -------------------------------------------------------

    #[test]
    fn rotates_and_shifts_three_generations() {
        let dir = temp_dir("rotate");
        let log = dir.join("desktop.log");
        std::fs::write(&log, vec![b'x'; 32]).unwrap();
        std::fs::write(rotated_path(&log, 1), "one").unwrap();
        std::fs::write(rotated_path(&log, 2), "two").unwrap();

        assert!(rotate_log(&log, 16, 3).unwrap());

        assert!(!log.exists(), "the live log must be gone so a fresh one starts");
        assert_eq!(std::fs::read_to_string(rotated_path(&log, 1)).unwrap(), "x".repeat(32));
        assert_eq!(std::fs::read_to_string(rotated_path(&log, 2)).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(rotated_path(&log, 3)).unwrap(), "two");
    }

    #[test]
    fn never_keeps_more_than_three_generations() {
        let dir = temp_dir("rotate-drop");
        let log = dir.join("desktop.log");
        std::fs::write(&log, "live").unwrap();
        std::fs::write(rotated_path(&log, 1), "one").unwrap();
        std::fs::write(rotated_path(&log, 2), "two").unwrap();
        std::fs::write(rotated_path(&log, 3), "oldest").unwrap();

        assert!(rotate_log(&log, 0, ROTATED_GENERATIONS).unwrap());

        assert_eq!(std::fs::read_to_string(rotated_path(&log, 1)).unwrap(), "live");
        assert_eq!(std::fs::read_to_string(rotated_path(&log, 2)).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(rotated_path(&log, 3)).unwrap(), "two");
        assert!(!rotated_path(&log, 4).exists());
    }

    #[test]
    fn small_or_missing_logs_are_left_alone() {
        let dir = temp_dir("rotate-small");
        let log = dir.join("desktop.log");
        std::fs::write(&log, "tiny").unwrap();

        assert!(!rotate_log(&log, MAX_LOG_BYTES, ROTATED_GENERATIONS).unwrap());
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "tiny");
        assert!(!rotated_path(&log, 1).exists());

        let missing = dir.join("nothing.log");
        assert!(!rotate_log(&missing, MAX_LOG_BYTES, ROTATED_GENERATIONS).unwrap());
    }

    #[test]
    fn open_rotating_starts_a_fresh_file() {
        let dir = temp_dir("open-rotating");
        let log = dir.join("desktop.log");
        std::fs::write(&log, "a".repeat(64)).unwrap();
        // Rotate manually through the public API, then prove the logger appends
        // to a brand-new file instead of the rotated one.
        assert!(rotate_log(&log, 16, ROTATED_GENERATIONS).unwrap());
        let logger = Logger::new(&log.display().to_string()).unwrap();
        logger.info("hello");
        assert!(std::fs::read_to_string(&log).unwrap().contains("hello"));
        assert_eq!(std::fs::read_to_string(rotated_path(&log, 1)).unwrap(), "a".repeat(64));
    }

    // ---- panic path -----------------------------------------------------

    #[test]
    fn panic_line_names_the_payload_and_location() {
        assert_eq!(
            format_panic_line("boom", Some(("src/lib.rs", 42))),
            "PANIC boom at src/lib.rs:42"
        );
        assert_eq!(
            format_panic_line("boom", None),
            "PANIC boom at <unknown location>"
        );
    }

    #[test]
    fn panic_sibling_sits_beside_the_log() {
        assert_eq!(
            panic_sibling_path(Path::new("/tmp/logs/desktop.log"), 1_790_446_908),
            PathBuf::from("/tmp/logs/panic-20260926T182148Z.log")
        );
        assert_eq!(
            panic_sibling_path(Path::new("logs/desktop.log"), 0),
            PathBuf::from("logs/panic-19700101T000000Z.log")
        );
    }

    #[test]
    fn append_panic_writes_the_log_and_a_sibling() {
        let dir = temp_dir("panic");
        let log = dir.join("desktop.log");
        set_panic_log_path(log.clone()); // first call wins for the whole test binary
        let text = "1970-01-01T00:00:00Z PANIC boom at src/lib.rs:42\nbacktrace\n";
        append_panic(text);
        assert_eq!(std::fs::read_to_string(&log).unwrap(), text);
        let sibling = panic_sibling_path(&log, unix_now());
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), text);
    }

    // ---- run events -----------------------------------------------------

    #[test]
    fn focus_and_scale_events_are_noisy() {
        for debug in [
            "Focused(true)",
            "Focused(false)",
            "ScaleFactorChanged { scale_factor: 2.0, new_inner_size: PhysicalSize { width: 1, height: 1 } }",
            "Moved(PhysicalPosition { x: 1, y: 2 })",
            "Resized(PhysicalSize { width: 1100, height: 780 })",
        ] {
            assert!(is_noisy_window_event(debug), "{debug} must not be logged");
        }
        for debug in ["Destroyed", "CloseRequested { api: CloseRequestApi }"] {
            assert!(!is_noisy_window_event(debug), "{debug} must be logged");
        }
        assert_eq!(event_variant_name("Focused(true)"), "Focused");
        assert_eq!(event_variant_name("Destroyed"), "Destroyed");
    }

    // ---- run-event whitelist --------------------------------------------

    #[test]
    fn only_informative_run_events_are_logged() {
        for debug in [
            "Ready",
            "Exit",
            "Resumed",
            "ExitRequested { code: None, api: ExitRequestApi(Sender { .. }) }",
            "WindowEvent { label: \"main\", event: Destroyed }",
            "WebviewEvent { label: \"webui\", event: DragDrop(DragDropEvent { .. }) }",
        ] {
            assert!(is_logged_run_event(debug), "{debug} must be logged");
        }
        for debug in [
            "MainEventsCleared",
            "MenuEvent(MenuEvent { .. })",
            "TrayIconEvent(TrayIconEvent { .. })",
        ] {
            assert!(!is_logged_run_event(debug), "{debug} must not be logged");
        }
    }

    // ---- redaction ------------------------------------------------------

    #[test]
    fn redacts_secret_values_in_every_shape_we_log() {
        assert_eq!(redact_secrets("token=abc123"), "token=***");
        assert_eq!(redact_secrets("access_token: abc123"), "access_token: ***");
        assert_eq!(
            redact_secrets(r#"{"refreshToken": "abc123"}"#),
            r#"{"refreshToken": "***"}"#
        );
        assert_eq!(
            redact_secrets("https://x/y?code=1&secret=abc123&z=2"),
            "https://x/y?code=1&secret=***&z=2"
        );
    }

    #[test]
    fn redaction_leaves_ordinary_text_alone() {
        for text in [
            "backend resolved: source=bundled",
            "session: locale=zh",
            "no assignment here",
            "口令 passwordless login",
        ] {
            assert_eq!(redact_secrets(text), text);
        }
        // Chinese text (multi-byte) must survive byte-safe iteration.
        assert_eq!(redact_secrets("端口已被占用"), "端口已被占用");
    }

    #[test]
    fn truncation_is_char_safe() {
        assert_eq!(truncate_chars("abc", 5), "abc");
        assert_eq!(truncate_chars("abcdef", 3), "abc…");
        // cutting inside a multi-byte character must not panic
        assert_eq!(truncate_chars("端口已被占用", 2), "端口…");
    }

    // ---- last-run bookkeeping -------------------------------------------

    #[test]
    fn last_run_round_trips_and_omits_exited_at() {
        let dir = temp_dir("last-run");
        let run = LastRun {
            started_at: 1_790_446_908,
            clean_exit: false,
            pid: 4242,
            version: "0.2.0".into(),
            exited_at: None,
        };
        write_last_run(&dir, &run).unwrap();
        let raw = std::fs::read_to_string(last_run_path(&dir)).unwrap();
        assert!(raw.contains("\"startedAt\": 1790446908"), "{raw}");
        assert!(raw.contains("\"cleanExit\": false"), "{raw}");
        assert!(!raw.contains("exitedAt"), "{raw}");
        assert_eq!(read_last_run(&dir).unwrap(), run);

        let finished = LastRun { clean_exit: true, exited_at: Some(1_790_446_999), ..run };
        write_last_run(&dir, &finished).unwrap();
        assert_eq!(read_last_run(&dir).unwrap().exited_at, Some(1_790_446_999));
    }

    #[test]
    fn a_missing_or_broken_last_run_is_none() {
        let dir = temp_dir("last-run-bad");
        assert!(read_last_run(&dir).is_none());
        std::fs::write(last_run_path(&dir), "not json").unwrap();
        assert!(read_last_run(&dir).is_none());
    }

    // ---- crash-report collection ----------------------------------------

    #[test]
    fn picks_the_newest_matching_crash_report() {
        let dir = temp_dir("crash");
        std::fs::write(dir.join("pixivflow-desktop-old.ips"), "old").unwrap();
        std::fs::write(dir.join("pixivflow-desktop-new.ips"), "new").unwrap();
        std::fs::write(dir.join("other-app-1.ips"), "other").unwrap();
        std::fs::write(dir.join("pixivflow-desktop-1.txt"), "not an ips").unwrap();
        // Distinguish the two candidates by modification time.
        let older = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        File::options()
            .write(true)
            .open(dir.join("pixivflow-desktop-old.ips"))
            .unwrap()
            .set_modified(older)
            .unwrap();

        let found = newest_crash_report_in(&dir, unix_now()).unwrap();
        assert_eq!(found.file_name().unwrap(), "pixivflow-desktop-new.ips");
    }

    #[test]
    fn reports_older_than_the_cutoff_are_ignored() {
        let dir = temp_dir("crash-cutoff");
        std::fs::write(dir.join("pixivflow-desktop-ancient.ips"), "old").unwrap();
        let very_old = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        File::options()
            .write(true)
            .open(dir.join("pixivflow-desktop-ancient.ips"))
            .unwrap()
            .set_modified(very_old)
            .unwrap();

        // Prev run started long after the report: it cannot belong to that run.
        assert!(newest_crash_report_in(&dir, unix_now()).is_none());
        // ... but 60 s of slack means "just before the previous run" still counts.
        let mtime = dir
            .join("pixivflow-desktop-ancient.ips")
            .metadata()
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(newest_crash_report_in(&dir, mtime + 30).is_some());
    }

    #[test]
    fn a_missing_reports_directory_is_not_an_error() {
        let dir = temp_dir("crash-missing");
        assert!(newest_crash_report_in(&dir.join("nope"), unix_now()).is_none());
    }
}
