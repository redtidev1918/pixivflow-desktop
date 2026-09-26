//! Host side of "tell the user something happened" (`notify`).
//!
//! A notification is a *device* action: it lives on the screen in front of the
//! user, and the PixivFlow backend has no screen. The WebUI decides *whether*
//! something is worth interrupting for and hands over copy it has already
//! localised; this module decides whether this machine can show it at all, and
//! starts the thing that does. It never composes user-visible text of its own —
//! a host that owned the wording would need a copy of every locale file the
//! WebUI has.
//!
//! The answer is deliberately three-valued, because "the user saw it" is not
//! something this layer may assume:
//!
//! - shown — a notification is on screen now;
//! - denied — the platform refused it (Do Not Disturb, a user setting);
//! - unavailable — this build cannot show one on this platform, which must be
//!   an honest coded answer rather than a quiet success. Reporting a
//!   notification nobody saw is worse than the silent nothing it replaced.
//!
//! Like `reveal`, the platform matrix is a pure function so the whole
//! invocation table is unit-tested on one machine instead of only on the machine
//! that happens to run it.

use std::process::Command;

/// The platform refused to display the notification (Do Not Disturb, Focus, a
/// per-app setting). The channel exists; the user is not being reached.
pub const DENIED: &str = "NOTIFY_DENIED";
/// This build has no way to show a system notification on this platform.
pub const UNAVAILABLE: &str = "NOTIFY_UNAVAILABLE";

/// What happened, in the shape the WebUI's capability layer expects.
///
/// `reason` carries the platform's coded answer (`NOTIFY_DENIED`,
/// `NOTIFY_UNAVAILABLE`); the bridge maps it onto the two reasons the WebUI
/// understands, and any other value means "nothing was shown".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Outcome {
    pub shown: bool,
    pub reason: Option<String>,
}

impl Outcome {
    pub fn shown() -> Self {
        Self {
            shown: true,
            reason: None,
        }
    }

    fn refused(reason: &str) -> Self {
        Self {
            shown: false,
            reason: Some(reason.to_string()),
        }
    }
}

/// The platform whose notification system this build talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

/// Which platform this build was compiled for.
///
/// `cfg!` (not `#[cfg]`) so every branch is compiled on every target and the
/// matrix is testable on one machine.
pub fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

/// What to start, or why nothing can be started.
///
/// `Unavailable` is a compile-time fact about the platform, not a runtime
/// failure: Windows needs a registered AppUserModelID and the WinRT toast API,
/// which is a plugin-scale dependency rather than a spawned program — so on
/// Windows this build answers honestly instead of doing nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Spawn { program: String, args: Vec<String> },
    Unavailable(&'static str),
}

/// The program and arguments that show `body` under `title` on `platform`.
///
/// Both strings travel as **argv**, never interpolated into a script: Tauri
/// spawns the program directly (no shell), so a title containing quotes,
/// backslashes or a newline cannot become code on any of these platforms.
pub fn invocation(platform: Platform, title: &str, body: &str) -> Plan {
    match platform {
        Platform::Macos => Plan::Spawn {
            program: "osascript".to_string(),
            // JavaScript for Automation (JXA): the copy arrives through
            // `run(argv)`, so AppleScript string escaping is not needed at all.
            args: vec![
                "-l".to_string(),
                "JavaScript".to_string(),
                "-e".to_string(),
                MACOS_SCRIPT.to_string(),
                title.to_string(),
                body.to_string(),
            ],
        },
        Platform::Linux => Plan::Spawn {
            program: "notify-send".to_string(),
            args: vec![title.to_string(), body.to_string()],
        },
        Platform::Windows => Plan::Unavailable(
            "Windows notifications need a registered AppUserModelID (WinRT toast API)",
        ),
    }
}

/// Show `body` under `title` on this machine.
///
/// Never returns the OS's own message: the caller gets one of three answers it
/// can act on, and the details belong in the desktop log, not in a user-facing
/// report.
pub fn notify(platform: Platform, title: &str, body: &str) -> Outcome {
    let (program, args) = match invocation(platform, title, body) {
        Plan::Spawn { program, args } => (program, args),
        Plan::Unavailable(_) => return Outcome::refused(UNAVAILABLE),
    };

    match Command::new(&program).args(&args).status() {
        Ok(status) if status.success() => Outcome::shown(),
        Ok(_) => Outcome::refused(&refusal_for(platform)),
        Err(_) => Outcome::refused(&refusal_for(platform)),
    }
}

/// Map a failure onto the only reason this layer can distinguish.
///
/// A separate function so the rule is visible in one place: the OS reports *a*
/// failure, and the WebUI's two answers are "the channel exists but the user is
/// not reached" (denied) or "there is no channel" (unavailable).
fn refusal_for(platform: Platform) -> String {
    match platform {
        // `osascript` and `notify-send` exit non-zero when the notification
        // could not be delivered — Focus/DND, or a missing permission.
        Platform::Macos | Platform::Linux => DENIED.to_string(),
        Platform::Windows => UNAVAILABLE.to_string(),
    }
}

/// The macOS script, kept as a constant so the test can assert it uses argv and
/// not string interpolation.
const MACOS_SCRIPT: &str = "function run(argv) { var app = Application.currentApplication(); \
app.includeStandardAdditions = true; app.displayNotification(argv[1], { withTitle: argv[0] }); \
return 'shown'; }";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_passes_the_copy_as_argv_not_as_script_text() {
        let plan = invocation(Platform::Macos, "Done \"quoted\"", "line\nbreak");
        let Plan::Spawn { program, args } = plan else {
            panic!("macOS must have an invocation");
        };
        assert_eq!(program, "osascript");
        assert_eq!(&args[0..3], &["-l", "JavaScript", "-e"]);
        // The script itself must not contain the copy at all.
        assert!(!args[3].contains("quoted"));
        assert!(!args[3].contains("line"));
        assert_eq!(args[4], "Done \"quoted\"");
        assert_eq!(args[5], "line\nbreak");
    }

    #[test]
    fn linux_passes_the_copy_as_argv() {
        let Plan::Spawn { program, args } = invocation(Platform::Linux, "Done", "body") else {
            panic!("Linux must have an invocation");
        };
        assert_eq!(program, "notify-send");
        assert_eq!(args, vec!["Done".to_string(), "body".to_string()]);
    }

    #[test]
    fn windows_says_it_cannot_rather_than_doing_nothing() {
        let plan = invocation(Platform::Windows, "Done", "body");
        assert!(matches!(plan, Plan::Unavailable(_)));
        assert_eq!(notify(Platform::Windows, "Done", "body"), Outcome::refused(UNAVAILABLE));
    }

    #[test]
    fn refusal_maps_to_denied_where_a_channel_exists() {
        assert_eq!(refusal_for(Platform::Macos), DENIED);
        assert_eq!(refusal_for(Platform::Linux), DENIED);
        assert_eq!(refusal_for(Platform::Windows), UNAVAILABLE);
    }

    #[test]
    fn an_outcome_is_either_shown_or_coded() {
        assert_eq!(
            Outcome::shown(),
            Outcome { shown: true, reason: None }
        );
        assert_eq!(
            Outcome::refused(DENIED),
            Outcome { shown: false, reason: Some(DENIED.to_string()) }
        );
    }
}
