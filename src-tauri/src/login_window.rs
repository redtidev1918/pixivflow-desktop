//! Pure URL helpers for the host login bridge.
//!
//! The desktop layer owns *native integration* only: it opens the Pixiv
//! authorize page, watches the resulting navigation, and hands the raw
//! `code` query parameter back to the WebUI. It never builds or interprets
//! a Pixiv URL, and it never talks to the Pixiv API — the backend owns the
//! whole OAuth flow (`POST /api/auth/login/host/start`).
//!
//! Everything here is deliberately dependency-free (no `url` crate, no
//! regex) and free of Tauri types so it is unit-testable headlessly.

/// Pixiv's OAuth callback path. The host accepts ANY origin/path ending in
/// this suffix so a backend that computes `redirectUri` slightly differently
/// (different host, extra prefix) still resolves — the exact `redirect_uri`
/// comparison has to stay lenient on that axis.
pub const AUTH_CALLBACK_PATH_SUFFIX: &str = "/web/v1/users/auth/pixiv/callback";

/// Extract the `code` query parameter of `url` when it is the OAuth callback
/// for `redirect_uri`.
///
/// Returns `None` for anything that is not a callback (see
/// [`is_callback_for`] for what counts) or that carries no usable `code`.
pub fn extract_auth_code(url: &str, redirect_uri: &str) -> Option<String> {
    let target = parse_url(url)?;
    let expected = parse_url(redirect_uri)?;
    if !is_callback_for(&target, &expected) {
        return None;
    }
    let code = first_query_param(&target.query, "code")
        .or_else(|| first_query_param(&target.fragment, "code"))?;
    if code.is_empty() {
        return None;
    }
    Some(code)
}

/// Leniently decide whether `target` is the OAuth callback for `redirect_uri`.
///
/// Accepted when either:
/// - `target` has the same scheme+host+path as `redirect_uri` (the query
///   necessarily differs — that is where the `code` lives), or
/// - `target`'s path ends in [`AUTH_CALLBACK_PATH_SUFFIX`].
pub fn is_callback_for(target: &UrlParts, redirect: &UrlParts) -> bool {
    // Scheme matches case-insensitively; host is already lowercased by
    // `parse_url`; the path comparison is exact (a prefix lookalike such as
    // `/…/callback-extra` must NOT match).
    let same_origin_and_path = target.scheme.eq_ignore_ascii_case(&redirect.scheme)
        && target.host == redirect.host
        && target.path == redirect.path;
    same_origin_and_path || target.path.ends_with(AUTH_CALLBACK_PATH_SUFFIX)
}

/// The pieces of an absolute URL the login bridge cares about. `host`
/// includes the port, so a different port counts as a different origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlParts {
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub query: String,
    /// The `#…` payload, without the `#`. Usually empty; some OAuth flows
    /// deliver the code there instead of in the query.
    pub fragment: String,
}

/// Parse an absolute http(s) URL into [`UrlParts`], or `None` when it is not
/// one (relative URL, unsupported scheme, no authority).
pub fn parse_url(raw: &str) -> Option<UrlParts> {
    let raw = raw.trim();
    let (scheme, rest) = raw.split_once("://")?;
    if scheme.is_empty() {
        return None;
    }
    // Authority runs to the first `/`, `?` or `#`; everything before the
    // path starts there.
    let authority_end = rest
        .find(|c: char| c == '/' || c == '?' || c == '#')
        .unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    if authority.is_empty() {
        return None;
    }
    let (path_query, fragment) = match tail.split_once('#') {
        Some((before, frag)) => (before, frag),
        None => (tail, ""),
    };
    let (path, query) = match path_query.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path_query, ""),
    };
    Some(UrlParts {
        scheme: scheme.to_ascii_lowercase(),
        host: authority.to_ascii_lowercase(),
        path: if path.is_empty() { "/" } else { path }.to_string(),
        query: query.to_string(),
        fragment: fragment.to_string(),
    })
}

/// Value of `key` in a raw query string (or fragment), percent-decoded.
/// `None` when absent or when the key has no `=`; `Some("")` when present
/// but empty.
fn first_query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            Some(percent_decode(v))
        } else {
            None
        }
    })
}

/// Minimal percent-decoding for a query value, covering the encodings an
/// OAuth `code` (or a redirect URI embedded in a query) can carry: `%XX`
/// plus `+` for space.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 <= bytes.len() => {
                match (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                    (Some(hi), Some(lo)) => {
                        out.push(hi * 16 + lo);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REDIRECT: &str = "http://127.0.0.1:43110/web/v1/users/auth/pixiv/callback";

    #[test]
    fn matches_callback_url_and_returns_code() {
        let url = format!("{REDIRECT}?code=abc123&state=xyz");
        assert_eq!(extract_auth_code(&url, REDIRECT), Some("abc123".to_string()));
    }

    #[test]
    fn matches_with_extra_query_params() {
        let url = format!("{REDIRECT}?state=xyz&code=code-with-extra&extra=1");
        assert_eq!(
            extract_auth_code(&url, REDIRECT),
            Some("code-with-extra".to_string())
        );
    }

    #[test]
    fn rejects_prefix_lookalike() {
        // Navigated URL is the callback path with something appended: same
        // origin, but NOT the callback (no exact path match, no suffix match).
        let url = "http://127.0.0.1:43110/web/v1/users/auth/pixiv/callback-extra?code=abc123";
        assert_eq!(extract_auth_code(url, REDIRECT), None);
    }

    #[test]
    fn rejects_missing_code() {
        assert_eq!(extract_auth_code(REDIRECT, REDIRECT), None);
        let with_state = format!("{REDIRECT}?state=xyz");
        assert_eq!(extract_auth_code(&with_state, REDIRECT), None);
        let empty_code = format!("{REDIRECT}?code=");
        assert_eq!(extract_auth_code(&empty_code, REDIRECT), None);
    }

    #[test]
    fn rejects_different_origin_or_path() {
        // A different port whose path is NOT the callback path matches neither
        // leg of the contract (origin+path, or the callback path suffix).
        assert_eq!(
            extract_auth_code(
                "http://127.0.0.1:9999/web/v1/users/auth/pixiv/other?code=abc123",
                REDIRECT
            ),
            None
        );
        // Same origin, unrelated path.
        assert_eq!(
            extract_auth_code("http://127.0.0.1:43110/dashboard?code=abc123", REDIRECT),
            None
        );
        // Not an absolute URL at all.
        assert_eq!(
            extract_auth_code("/web/v1/users/auth/pixiv/callback?code=abc", REDIRECT),
            None
        );
    }

    #[test]
    fn a_different_origin_on_the_callback_path_is_accepted_by_design() {
        // The two legs of the contract are a union: origin+path equality OR the
        // callback path suffix. The suffix leg deliberately accepts another
        // origin, because the backend computes `redirectUri` and the desktop
        // must not second-guess which host the callback lands on.
        assert_eq!(
            extract_auth_code(
                "http://127.0.0.1:9999/web/v1/users/auth/pixiv/callback?code=abc123",
                REDIRECT
            ),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn accepts_any_origin_with_the_callback_path_suffix() {
        // The backend computes redirectUri; a different host/prefix on the
        // very same callback path still counts as the callback.
        let url = format!("https://pixiv.example/auth{0}?code=suffix-code", AUTH_CALLBACK_PATH_SUFFIX);
        assert_eq!(
            extract_auth_code(&url, REDIRECT),
            Some("suffix-code".to_string())
        );
    }

    #[test]
    fn extracts_code_from_fragment_and_decodes_percent_escapes() {
        // Some callbacks put the payload in the fragment instead of the query.
        let url = format!("{REDIRECT}#code=frag-code");
        assert_eq!(extract_auth_code(&url, REDIRECT), Some("frag-code".to_string()));
        // A percent-encoded value is decoded (an escape ending the value too).
        let encoded = format!("{REDIRECT}?code=a%2Bb%20c");
        assert_eq!(extract_auth_code(&encoded, REDIRECT), Some("a+b c".to_string()));
        let trailing = format!("{REDIRECT}?code=tail%2F");
        assert_eq!(extract_auth_code(&trailing, REDIRECT), Some("tail/".to_string()));
        // A query wins over a fragment when both are present.
        let both = format!("{REDIRECT}?code=query-code#code=frag-code");
        assert_eq!(extract_auth_code(&both, REDIRECT), Some("query-code".to_string()));
    }

    #[test]
    fn path_without_trailing_slash_defaults_to_root() {
        // A redirect_uri without a path is the root path on both sides.
        let target = parse_url("http://127.0.0.1:1/?code=x").unwrap();
        let redirect = parse_url("http://127.0.0.1:1").unwrap();
        assert_eq!(target.path, "/");
        assert_eq!(redirect.path, "/");
        assert!(is_callback_for(&target, &redirect));
    }

    #[test]
    fn different_query_is_what_makes_it_a_callback() {
        // The exact contract: same scheme+host+path, differing query (that is
        // where the code lives). A different path on the same origin is not
        // the callback.
        let target = parse_url(&format!("{REDIRECT}?code=1")).unwrap();
        let redirect = parse_url(REDIRECT).unwrap();
        assert_eq!(target.scheme, redirect.scheme);
        assert_eq!(target.host, redirect.host);
        assert_eq!(target.path, redirect.path);
        assert!(is_callback_for(&target, &redirect));
        let other =
            parse_url("http://127.0.0.1:43110/web/v1/users/auth/pixiv/other?code=1").unwrap();
        assert!(!is_callback_for(&other, &redirect));
    }
}
