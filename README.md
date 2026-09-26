# PixivFlow Desktop

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)]()
[![Made with Tauri 2](https://img.shields.io/badge/Made%20with-Tauri%202-purple)]()

> PixivFlow Desktop is a cross-platform desktop shell for **PixivFlow**.

It is **not**:

- a new version of PixivFlow,
- a rewrite of PixivFlow,
- nor a standalone client with its own business logic.

It is:

- a **Tauri wrapper** around the existing PixivFlow backend,
- a **lifecycle manager** for the local backend process,
- and the **WebUI integration** that opens pixivflow-webui once the backend is healthy.

Business logic stays in upstream PixivFlow. The desktop only launches, supervises,
contains and updates it.

## Status (F1 — Shell MVP)

Runnable Tauri 2 shell: launches a window, reads `desktop-config.json`, starts the
backend, health-checks `/api/health` and stops it gracefully on window close.
F1 ships a bundled **mock backend** as the health stand-in; the real PixivFlow
integration is Phase 2. See [docs/ROADMAP.md](/docs/ROADMAP.md) for the full
phase plan and [docs/DEVELOPMENT.md](/docs/DEVELOPMENT.md) to run it.

## Architecture

```
PixivFlow Desktop
        |
        |
     Tauri 2
        |
 ----------------
 |              |
WebUI        Backend
 |              |
pixivflow-webui PixivFlow
```

## Why Tauri 2?

- **Small package** — native webview, ~10–20 MB installers instead of 100 MB+.
- **Low memory** — OS native web engine, no bundled Chromium.
- **Cross-platform** — Windows / macOS / Linux from one codebase.
- **Rust process management** — small, auditable binary for backend lifecycle
  (start / stop / health check).
- **GitHub Releases updater** — first-class signed updates straight from GitHub
  Releases, no self-hosted updater server to operate.

## Supported platforms

| Platform | Installer |
|---|---|
| Windows 10/11 | `.exe` |
| macOS — Intel | `.dmg` |
| macOS — Apple Silicon | `.dmg` |
| Linux | `.AppImage` |

## Relationship to the ecosystem

```
PixivFlow
        |
pixivflow-webui
        |
pixivflow-desktop

releasegraph 管理版本关系
```

`releasegraph` orchestrates the versions and release pipelines of every repo in
the PixivFlow ecosystem. This repo's [`desktop-manifest.json`](desktop-manifest.json)
is the component version lock that releasegraph reads to drive the desktop's
automatic upgrade flow.

## Project status

**F0 — Foundation.** This is the approved repository skeleton: structure,
documentation and contracts only. No Tauri project has been initialized, no Rust
is written yet, and the backend is **not** started or packaged at this stage.
Business logic is intentionally absent.

See [docs/ROADMAP.md](docs/ROADMAP.md) for the phased plan (F0 → F5).

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Development](docs/DEVELOPMENT.md)
- [Roadmap](docs/ROADMAP.md)
- [Release & version management](docs/RELEASE.md)
- [Relationship to the ecosystem](docs/RELATIONSHIP.md)
- [Release-ecosystem analysis (input research)](docs/analysis/RELEASE-ECOSYSTEM-ANALYSIS.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues → [SECURITY.md](SECURITY.md).
Code of conduct → [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

MIT © [redtidev1918](https://github.com/redtidev1918). Upstream
[PixivFlow](https://github.com/redtidev1918/PixivFlow) and
[pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) carry their own
licenses — see their repositories.