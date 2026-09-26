# CI/CD workflows (reserved)

This directory is the home for the desktop's CI/CD pipelines. Per the roadmap
they are introduced in later phases to keep this repo's foundation thin:

- **F1 / build** — `build.yml`: cross-platform matrix (Windows `.exe`, macOS
  `.dmg`, Linux `.AppImage`) via Tauri; lint + `cargo check` on PRs.
- **F4 / signing** — attach signing & macOS notarization (self-signed fallback
  where unconfigured).
- **F5 / updater** — publish signed Tauri updater payloads to GitHub Releases.
- **F6 / release** — `release.yml`: a pinned call to releasegraph's shared
  `reusable-release.yml`, mirroring the PixivFlow / pixivflow-webui repos (no
  forking of stable workflows). Driven by bumps of `desktop-manifest.json`.

Until those land, builds are validated locally (`docs/DEVELOPMENT.md`) and the
upstream component pins live in `desktop-manifest.json`.