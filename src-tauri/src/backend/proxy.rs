//! System proxy discovery for the spawned backend.
//!
//! Started from Finder / the dock, the app inherits **no** proxy environment
//! variables, so the backend cannot reach Pixiv while the WKWebView login window
//! (which picks up the macOS system proxy on its own) can. The desktop therefore
//! reads the macOS system proxy via `scutil --proxy` and forwards it to the
//! backend child as `HTTPS_PROXY` / `HTTP_PROXY` — PixivFlow maps exactly those
//! variables into its own `network.proxy` configuration, so the API/token
//! exchange and the downloads stay on the same network path.
//!
//! This module only *describes* the environment to inject (pure parsing plus a
//! pure plan); applying it is the manager's job at spawn time. Discovery
//! priority and the process lifecycle are untouched.
//!
//! Nothing here ever logs or returns credentials: the system proxy is a plain
//! `host:port`.

/// Variables the backend understands and that we are willing to inject.
pub const PROXY_ENV_KEYS: [&str; 2] = ["HTTPS_PROXY", "HTTP_PROXY"];

/// Loopback must never go through the proxy: the desktop polls the backend's
/// own `/api/health` on `127.0.0.1`.
pub const NO_PROXY_VALUE: &str = "127.0.0.1,localhost,::1";

/// What the last spawn did about the system proxy — used for the log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyInjection {
    /// The system proxy was forwarded (contains `host:port`, never credentials).
    Injected(String),
    /// A system proxy exists, but the variables were already set (inherited env
    /// or the LaunchSpec override), so nothing was overridden.
    AlreadySet(String),
    /// No system proxy on this machine (or not macOS).
    Unavailable,
}

impl ProxyInjection {
    /// One-line, credential-free description for the desktop log.
    pub fn describe(&self) -> String {
        match self {
            ProxyInjection::Injected(p) => {
                format!("system proxy {p} injected as HTTPS_PROXY/HTTP_PROXY")
            }
            ProxyInjection::AlreadySet(p) => {
                format!("system proxy {p} found but HTTPS_PROXY/HTTP_PROXY already set — left as-is")
            }
            ProxyInjection::Unavailable => {
                "no system proxy found (HTTPS_PROXY/HTTP_PROXY left untouched)".to_string()
            }
        }
    }
}

/// The environment to set on the backend child, plus how to describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyPlan {
    /// Variables to set on the child process, in order.
    pub env: Vec<(&'static str, String)>,
    /// Outcome for the log line.
    pub outcome: ProxyInjection,
}

/// Decide which proxy variables to inject, without touching any process.
///
/// `is_set` reports whether a variable is already present in the inherited
/// environment or in the LaunchSpec override: an existing value is **never**
/// overridden, because that is the user's explicit choice.
pub fn plan_proxy(proxy: Option<String>, is_set: impl Fn(&str) -> bool) -> ProxyPlan {
    let mut env = Vec::new();
    let outcome = match proxy {
        Some(proxy) => {
            let mut injected_any = false;
            for key in PROXY_ENV_KEYS {
                if !is_set(key) {
                    env.push((key, proxy.clone()));
                    injected_any = true;
                }
            }
            if injected_any {
                ProxyInjection::Injected(proxy)
            } else {
                ProxyInjection::AlreadySet(proxy)
            }
        }
        None => ProxyInjection::Unavailable,
    };
    // Loopback stays direct. Set unconditionally (unless the user already
    // decided), so a proxy configured in PixivFlow's own config file can never
    // route the desktop's health checks through a proxy.
    if !is_set("NO_PROXY") && !is_set("no_proxy") {
        env.push(("NO_PROXY", NO_PROXY_VALUE.to_string()));
    }
    ProxyPlan { env, outcome }
}

/// Parse `scutil --proxy` output into `http://host:port`.
///
/// Prefers the HTTPS block, falls back to the HTTP block. Returns `None` when
/// neither is enabled, or when the enabled block is missing its host or a valid
/// port — an incomplete block never silently falls through to the other one.
pub fn parse_scutil_proxy(text: &str) -> Option<String> {
    let entries = scutil_entries(text);
    let blocks = [
        ("HTTPSEnable", "HTTPSProxy", "HTTPSPort"),
        ("HTTPEnable", "HTTPProxy", "HTTPPort"),
    ];
    for (enable_key, host_key, port_key) in blocks {
        match entries.get(enable_key).map(String::as_str) {
            Some("1") => {
                // Enabled: this is the block that decides. A missing or invalid
                // field is a hard `None`, not a fallback.
                let host = entries.get(host_key)?;
                let port = entries.get(port_key)?.parse::<u16>().ok()?;
                if host.is_empty() {
                    return None;
                }
                return Some(format!("http://{host}:{port}"));
            }
            _ => continue,
        }
    }
    None
}

/// Flatten the `key : value` lines of the `scutil --proxy` dictionary. Nested
/// arrays (`ExceptionsList`) only contribute their numeric index keys, which are
/// ignored by the caller.
fn scutil_entries(text: &str) -> std::collections::HashMap<String, String> {
    let mut entries = std::collections::HashMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key.is_empty() || value.is_empty() {
            continue;
        }
        entries.insert(key.to_string(), value.to_string());
    }
    entries
}

/// The machine's system HTTP proxy, in `http://host:port` form. macOS only;
/// every other platform returns `None`.
#[cfg(target_os = "macos")]
pub fn system_proxy() -> Option<String> {
    let output = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_scutil_proxy(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "macos"))]
pub fn system_proxy() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A realistic `scutil --proxy` dump (proxy enabled on 127.0.0.1:7892 with
    /// the usual loopback exceptions).
    const ENABLED: &str = r#"<dictionary> {
  ExceptionsList : <array> {
    0 : 127.0.0.1
    1 : localhost
    2 : 192.168.0.0/16
  }
  FTPPassive : 1
  HTTPEnable : 1
  HTTPPort : 7892
  HTTPProxy : 127.0.0.1
  HTTPSEnable : 1
  HTTPSPort : 7892
  HTTPSProxy : 127.0.0.1
  ProxyAutoConfigEnable : 0
  SOCKSEnable : 1
  SOCKSPort : 7892
  SOCKSProxy : 127.0.0.1
}"#;

    #[test]
    fn parses_the_enabled_https_block() {
        assert_eq!(parse_scutil_proxy(ENABLED), Some("http://127.0.0.1:7892".into()));
    }

    #[test]
    fn falls_back_to_the_http_block() {
        let text = r#"<dictionary> {
  HTTPEnable : 1
  HTTPPort : 8080
  HTTPProxy : proxy.example.com
  HTTPSEnable : 0
  HTTPSPort : 7892
  HTTPSProxy : 127.0.0.1
}"#;
        assert_eq!(
            parse_scutil_proxy(text),
            Some("http://proxy.example.com:8080".into())
        );
    }

    #[test]
    fn disabled_entries_are_none() {
        let text = "<dictionary> {\n  HTTPEnable : 0\n  HTTPProxy : 127.0.0.1\n  HTTPPort : 7892\n  HTTPSEnable : 0\n  HTTPSProxy : 127.0.0.1\n  HTTPSPort : 7892\n}";
        assert_eq!(parse_scutil_proxy(text), None);
    }

    #[test]
    fn missing_port_is_none() {
        let text = "<dictionary> {\n  HTTPSEnable : 1\n  HTTPSProxy : 127.0.0.1\n}";
        assert_eq!(parse_scutil_proxy(text), None);
    }

    #[test]
    fn enabled_but_incomplete_block_does_not_fall_through() {
        // HTTPS is the enabled entry, so its missing port is the answer — the
        // disabled-by-omission HTTP block must not be used instead.
        let text = "<dictionary> {\n  HTTPEnable : 1\n  HTTPProxy : proxy.example.com\n  HTTPPort : 8080\n  HTTPSEnable : 1\n  HTTPSProxy : 127.0.0.1\n}";
        assert_eq!(parse_scutil_proxy(text), None);
    }

    #[test]
    fn non_numeric_port_is_none() {
        let text = "<dictionary> {\n  HTTPSEnable : 1\n  HTTPSProxy : 127.0.0.1\n  HTTPSPort : eight\n}";
        assert_eq!(parse_scutil_proxy(text), None);
    }

    #[test]
    fn empty_and_garbage_input_is_none() {
        for text in ["", "   ", "no proxy here", "<dictionary> {\n}"] {
            assert_eq!(parse_scutil_proxy(text), None, "input: {text:?}");
        }
    }

    #[test]
    fn plan_injects_both_variables_and_no_proxy() {
        let plan = plan_proxy(Some("http://127.0.0.1:7892".into()), |_| false);
        assert_eq!(
            plan.env,
            vec![
                ("HTTPS_PROXY", "http://127.0.0.1:7892".to_string()),
                ("HTTP_PROXY", "http://127.0.0.1:7892".to_string()),
                ("NO_PROXY", NO_PROXY_VALUE.to_string()),
            ]
        );
        assert_eq!(
            plan.outcome,
            ProxyInjection::Injected("http://127.0.0.1:7892".into())
        );
    }

    #[test]
    fn plan_never_overrides_existing_variables() {
        // The user (or the LaunchSpec) already set both: nothing is injected and
        // the log says so.
        let plan = plan_proxy(Some("http://127.0.0.1:7892".into()), |key| {
            PROXY_ENV_KEYS.contains(&key)
        });
        assert_eq!(plan.env, vec![("NO_PROXY", NO_PROXY_VALUE.to_string())]);
        assert_eq!(
            plan.outcome,
            ProxyInjection::AlreadySet("http://127.0.0.1:7892".into())
        );
    }

    #[test]
    fn plan_keeps_an_existing_no_proxy() {
        let plan = plan_proxy(None, |key| key == "no_proxy");
        assert!(plan.env.is_empty());
        assert_eq!(plan.outcome, ProxyInjection::Unavailable);
    }

    #[test]
    fn plan_reports_unavailable_without_a_proxy_but_still_pins_loopback() {
        let plan = plan_proxy(None, |_| false);
        assert_eq!(plan.env, vec![("NO_PROXY", NO_PROXY_VALUE.to_string())]);
        assert_eq!(plan.outcome, ProxyInjection::Unavailable);
        assert!(plan.outcome.describe().contains("no system proxy"));
    }

    #[test]
    fn describe_never_leaks_more_than_host_and_port() {
        let described = ProxyInjection::Injected("http://127.0.0.1:7892".into()).describe();
        assert!(described.contains("127.0.0.1:7892"));
        assert!(!described.contains("//user:"));
    }
}
