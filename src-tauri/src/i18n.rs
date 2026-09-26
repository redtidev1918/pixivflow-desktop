//! Minimal launcher localization.
//!
//! The launcher UI is localized in the frontend (`src/frontend/i18n.js`); this
//! module is the system-language source of truth it asks through `get_locale`,
//! and it localizes the handful of strings the Rust side owns (window titles,
//! command errors).
//!
//! Rule: every user-visible string must come from here or from
//! `src/frontend/i18n.js` — never hard-coded at the call site.

use std::sync::OnceLock;

/// Environment override, mainly for support and for testing the other locale:
/// `PIXIVFLOW_DESKTOP_LOCALE=zh`.
pub const LOCALE_ENV: &str = "PIXIVFLOW_DESKTOP_LOCALE";

/// The system locale, or `None` when it cannot be determined.
fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

/// Normalize any locale tag to one of our two supported locales:
/// `zh`, `zh-Hans-CN`, `zh_CN`, `zh-TW` -> `zh`; everything else -> `en`.
pub fn normalize_locale(raw: &str) -> &'static str {
    let lower = raw.trim().to_ascii_lowercase();
    let primary = lower.split(['-', '_']).next().unwrap_or("");
    if primary == "zh" { "zh" } else { "en" }
}

fn resolve_locale() -> &'static str {
    if let Ok(raw) = std::env::var(LOCALE_ENV) {
        if !raw.trim().is_empty() {
            return normalize_locale(&raw);
        }
    }
    match system_locale() {
        Some(raw) => normalize_locale(&raw),
        None => "en",
    }
}

/// The resolved locale, computed once per process and cached.
pub fn locale() -> &'static str {
    static LOCALE: OnceLock<&'static str> = OnceLock::new();
    LOCALE.get_or_init(resolve_locale)
}

pub fn is_zh() -> bool {
    locale() == "zh"
}

/// Pick the Chinese or English variant for the current locale.
///
/// Allocates: the messages that need this carry a formatted value (a port
/// number), so a borrowed return would not compile at the call site. Use
/// [`t_static`] when both variants are literals.
pub fn t(zh: &str, en: &str) -> String {
    if is_zh() { zh.to_string() } else { en.to_string() }
}

/// [`t`] for literal templates, without the allocation.
pub fn t_static(zh: &'static str, en: &'static str) -> &'static str {
    if is_zh() { zh } else { en }
}

/// The launcher's locale, for `invoke("get_locale")` in `src/frontend/main.js`.
#[tauri::command]
pub fn get_locale() -> String {
    locale().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_chinese_variants() {
        for raw in ["zh", "zh-CN", "zh_CN", "zh-Hans-CN", "zh-Hant-TW", "ZH", " zh-Hans "] {
            assert_eq!(normalize_locale(raw), "zh", "{raw} should be Chinese");
        }
    }

    #[test]
    fn everything_else_falls_back_to_english() {
        for raw in ["en", "en-US", "ja-JP", "fr", "", "   ", "de-DE"] {
            assert_eq!(normalize_locale(raw), "en", "{raw} should be English");
        }
    }

    #[test]
    fn locale_is_resolved_and_cached() {
        let first = locale();
        assert!(first == "zh" || first == "en", "unexpected locale {first}");
        assert_eq!(first, locale());
        assert_eq!(is_zh(), first == "zh");
    }

    #[test]
    fn t_picks_the_variant_for_the_resolved_locale() {
        let zh = t("端口", "port");
        let en = t("端口", "port");
        if is_zh() {
            assert_eq!(zh, "端口");
            assert_eq!(en, "端口");
        } else {
            assert_eq!(zh, "port");
            assert_eq!(en, "port");
        }
        assert_eq!(get_locale(), locale());
    }

    #[test]
    fn t_static_returns_the_same_pick() {
        let picked = t_static("Pixiv 登录", "Pixiv Sign-in");
        if is_zh() {
            assert_eq!(picked, "Pixiv 登录");
        } else {
            assert_eq!(picked, "Pixiv Sign-in");
        }
    }
}
