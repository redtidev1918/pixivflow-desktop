//! Host side of the two link promises (`openExternal` / `openUrl`).
//!
//! These are deliberately separate capabilities and must not be conflated:
//!
//! - `open_external` hands a link to the OS so it opens in the **user's own
//!   browser**. This is what a "made with PixivFlow" link, a docs link or an
//!   OAuth page needs: the page is outside this app, in a real browser tab the
//!   user can see the address of.
//! - `open_in_app` navigates the **WebUI window** to a link, which is what a
//!   link to another PixivFlow page needs. It is confined to the app's own
//!   origin: a page served from the backend must not be able to turn the shell
//!   into a browser showing something else.
//!
//! Both accept only `http`/`https`. Everything else — `javascript:`, `file:`,
//! `data:` — is refused at the door rather than judged case by case, because the
//! caller that matters here is a remote page whose input is not fully trusted.
//!
//! As in `reveal` and `notify`, the platform matrix is a pure function so the
//! invocation table is unit-tested on one machine.

use std::process::Command;

/// The address is not a usable `http(s)` URL (or not one this app may open).
pub const LINK_INVALID: &str = "LINK_INVALID";
/// This platform has no way to hand a link to the user's browser.
pub const LINK_UNAVAILABLE: &str = "LINK_UNAVAILABLE";
/// The OS refused to start a browser, or it exited non-zero.
pub const LINK_FAILED: &str = "LINK_FAILED";

/// The platform whose browser launcher this build talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

/// Which platform this build was compiled for.
///
/// `cfg!` (not `#[cfg]`) so every branch is compiled on every target.
pub fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

/// `Some(())` when the address may be opened at all, `None` otherwise.
///
/// Only `http`/`https` survive. A URL with no scheme (`example.com`) or a
/// non-hierarchical one (`mailto:`) is refused, so a caller cannot smuggle a
/// protocol handler through this layer.
pub fn is_openable(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Whether `url` points at the app the WebUI window is already showing.
///
/// Scheme, host and port must all match: `http://127.0.0.1:3000` is the app,
/// `http://127.0.0.1:9999` is a different local server and is refused.
pub fn is_same_origin(current: &str, url: &str) -> bool {
    match (origin_of(current), origin_of(url)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// `scheme://host:port` in lower case, with the scheme's default port filled in
/// so `http://127.0.0.1` and `http://127.0.0.1:80` compare equal.
fn origin_of(url: &str) -> Option<String> {
    let rest = url.trim().split_once("://")?;
    let scheme = rest.0.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.1.split(['/', '?', '#']).next()?;
    if authority.is_empty() {
        return None;
    }
    // Drop any userinfo: it does not decide the origin and must not be trusted.
    let authority = authority.rsplit('@').next()?;
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
            (host, Some(port.to_string()))
        }
        _ => (authority, None),
    };
    if host.is_empty() {
        return None;
    }
    let port = port.unwrap_or_else(|| if scheme == "https" { "443" } else { "80" }.to_string());
    Some(format!("{}://{}:{}", scheme, host.to_ascii_lowercase(), port))
}

/// What to start, or why nothing can be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Spawn { program: String, args: Vec<String> },
    Unavailable(&'static str),
}

/// The program and arguments that show `url` in the user's browser on
/// `platform`.
///
/// The URL travels as **argv**, never through a shell, so a query string
/// containing `&`, `"` or `$` cannot become a second command.
pub fn invocation(platform: Platform, url: &str) -> Plan {
    match platform {
        Platform::Macos => Plan::Spawn {
            program: "open".to_string(),
            args: vec![url.to_string()],
        },
        Platform::Linux => Plan::Spawn {
            // `--` ends option parsing: a URL can never be read as a flag,
            // which the OS filter above already prevents but does not need to
            // be the only thing standing between a URL and `x-www-browser`.
            program: "xdg-open".to_string(),
            args: vec!["--".to_string(), url.to_string()],
        },
        Platform::Windows => Plan::Unavailable(
            "Windows has no launcher that takes a URL as argv without a shell (cmd /C start)",
        ),
    }
}

/// Hand `url` to the user's browser. Returns the coded error the WebUI expects.
pub fn open_external(platform: Platform, url: &str) -> Result<(), String> {
    if !is_openable(url) {
        return Err(LINK_INVALID.to_string());
    }
    let (program, args) = match invocation(platform, url) {
        Plan::Spawn { program, args } => (program, args),
        Plan::Unavailable(_) => return Err(LINK_UNAVAILABLE.to_string()),
    };
    // A browser launcher returns as soon as the hand-off is done; waiting is
    // cheap and gives an honest answer about whether anything happened.
    match Command::new(&program).args(&args).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(LINK_FAILED.to_string()),
        Err(_) => Err(LINK_FAILED.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_and_https_are_openable() {
        assert!(is_openable("http://127.0.0.1:3000/dashboard"));
        assert!(is_openable("https://example.com/a?b=1&c=2"));
        assert!(is_openable("HTTPS://EXAMPLE.COM"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("file:///etc/passwd"));
        assert!(!is_openable("data:text/html,<script>1</script>"));
        assert!(!is_openable("mailto:someone@example.com"));
        assert!(!is_openable("example.com"));
        assert!(!is_openable(""));
    }

    #[test]
    fn same_origin_needs_scheme_host_and_port_to_match() {
        assert!(is_same_origin("http://127.0.0.1:3000/dashboard", "http://127.0.0.1:3000/logs"));
        assert!(is_same_origin("https://example.com", "https://example.com/x"));
        // Default port is filled in, so these are the same origin.
        assert!(is_same_origin("https://example.com/a", "https://example.com:443/b"));
        assert!(!is_same_origin("http://127.0.0.1:3000", "http://127.0.0.1:3999"));
        assert!(!is_same_origin("http://127.0.0.1:3000", "https://127.0.0.1:3000"));
        assert!(!is_same_origin("http://127.0.0.1:3000", "http://localhost:3000"));
        assert!(!is_same_origin("http://127.0.0.1:3000", "javascript://127.0.0.1:3000"));
        assert!(!is_same_origin("", "http://127.0.0.1:3000"));
    }

    #[test]
    fn userinfo_does_not_decide_the_origin() {
        assert!(is_same_origin("http://127.0.0.1:3000", "http://evil@127.0.0.1:3000/x"));
    }

    #[test]
    fn macos_passes_the_url_as_argv() {
        let Plan::Spawn { program, args } = invocation(Platform::Macos, "https://a/b?c=1&d=2") else {
            panic!("macOS must have an invocation");
        };
        assert_eq!(program, "open");
        assert_eq!(args, vec!["https://a/b?c=1&d=2".to_string()]);
    }

    #[test]
    fn linux_ends_option_parsing_before_the_url() {
        let Plan::Spawn { program, args } = invocation(Platform::Linux, "https://a/b") else {
            panic!("Linux must have an invocation");
        };
        assert_eq!(program, "xdg-open");
        assert_eq!(args, vec!["--".to_string(), "https://a/b".to_string()]);
    }

    #[test]
    fn windows_says_it_cannot_rather_than_going_through_a_shell() {
        assert!(matches!(invocation(Platform::Windows, "https://a"), Plan::Unavailable(_)));
        assert_eq!(
            open_external(Platform::Windows, "https://a"),
            Err(LINK_UNAVAILABLE.to_string())
        );
    }

    #[test]
    fn invalid_urls_are_refused_before_any_spawn() {
        assert_eq!(
            open_external(Platform::Macos, "javascript:alert(1)"),
            Err(LINK_INVALID.to_string())
        );
        assert_eq!(open_external(Platform::Macos, ""), Err(LINK_INVALID.to_string()));
    }
}
