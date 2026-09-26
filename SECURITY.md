# Security Policy

## Supported versions

PixivFlow Desktop ships as a desktop shell that bundles two upstream
components (`PixivFlow` backend and `pixivflow-webui`). Security support
tracks the latest release for each component lock; older desktop releases are
supported **only** insofar as their bundled components remain supported.

## Reporting a vulnerability

**Do not open a public issue.** Report privately via a GitHub private
security advisory, or open a security disclosure to the maintainer of the
PixivFlow ecosystem.

Please include:

- Affected version(s) and operating system(s).
- A concise description and, if possible, a minimal reproduction.
- (If known) the impacted component: desktop shell vs. bundled backend vs. WebUI.

## What we consider

- The desktop shell spawns the backend on a loopback host (`127.0.0.1`) by
  default. **Keep it loopback** unless you explicitly configure backend auth.
- The main window loads the bundled WebUI origin; Tauri capabilities are
  limited to `core:` permissions on shell windows (see
  `src-tauri/capabilities/default.json`) so an XSS in the WebUI cannot call
  arbitrary native APIs.
- We do **not** run an additional self-hosted update server; updates come from
  GitHub Releases over HTTPS with Tauri updater signatures.

## Response

We aim to acknowledge receipt within 5 business days and will coordinate
disclosure with you. You will be credited unless you ask otherwise.