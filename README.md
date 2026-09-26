# PixivFlow Desktop

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)]()
[![Made with Tauri 2](https://img.shields.io/badge/Made%20with-Tauri%202-purple)]()

> PixivFlow Desktop is the **native desktop distribution** for **PixivFlow** —
> the official desktop runtime environment and release layer.

It is **not**:

- a new version of PixivFlow,
- a rewrite of PixivFlow,
- nor a standalone client with its own business logic.

It is:

- a **one-click native launcher** that installs, starts and packages the PixivFlow
  runtime for ordinary users,
- a **lifecycle manager** for the local PixivFlow backend process,
- and the **WebUI distribution** that opens pixivflow-webui once the backend is healthy.

Business logic stays in upstream PixivFlow. The desktop only launches, supervises,
bundles and updates it.

## Features

- **Open-and-run (one-click launch)** — double-click the app: it reads its config,
  auto-starts the bundled PixivFlow backend, health-checks it and opens the WebUI.
- **Native desktop experience** — a fast, self-contained Tauri 2 app that boots
  straight into a PixivFlow Desktop bootstrap UI (never a blank window).
- **Local backend lifecycle management** — the Rust `BackendManager` starts,
  stops (graceful SIGTERM) and restarts the local backend process, and manages
  its runtime discovery (bundled → config → PATH → dev mock fallback).
- **Bundled runtime** — ships a packaged PixivFlow backend
  (`resources/runtime/pixivflow/`, described by `runtime-manifest.json`) and the
  WebUI dist, so an ordinary user needs no separate install.
- **Runtime status monitoring** — live port / PID / health readout fed by the
  `/api/health` probe and pushed to the UI via the `backend-status` event.
- **Configuration** — reads `desktop-config.json` (auto-created with defaults)
  for backend port, auto-start, and command override.
- **WebUI integration (方案 A)** — the desktop opens the bundled WebUI directly
  over `http://127.0.0.1:{port}/`, served by the backend via `STATIC_PATH` (no
  second frontend). A dedicated `webui` window is created/reused on demand.

## Status (F2.2 — Bundled runtime + WebUI integration)

Runnable Tauri 2 desktop app that discovers and supervises a **bundled** backend
(`resources/runtime/pixivflow/`, described by `runtime-manifest.json`) and opens
the **bundled WebUI** (`resources/webui/dist`) served by that backend — 方案 A.
Discovery precedence keeps a shipped install stable: bundled → config → PATH →
dev mock fallback. Core `BackendManager` stays a pure lifecycle owner. The
control UI shows backend status / port / PID / health / source / version and
offers **打开 PixivFlow** (open WebUI), Restart, Stop, Open Logs. A versioned
`dev-backend.mjs` stand-in proves the full bundled contract until the real
release binary is wired (F2.3). See [docs/ROADMAP.md](/docs/ROADMAP.md) for the
full phase plan and [docs/DEVELOPMENT.md](/docs/DEVELOPMENT.md) to run it.

## Architecture

```
PixivFlow Desktop
      |
      v
PixivFlow Backend
      |
      v
PixivFlow WebUI
```

Three layers, one responsibility each: the **Desktop** (this repo, Tauri 2 +
Rust) manages the runtime; the **Backend** (upstream PixivFlow) owns all
business logic; the **WebUI** (upstream pixivflow-webui) is the interface the
desktop opens. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

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

**F2.2 — Bundled runtime + WebUI integration (方案 A).** F0 (foundation) → F1
(Desktop Shell MVP) → F1.1 (boot experience, white-screen fix, product naming)
→ F2.1 (real backend adapter/discovery/doctor) → F2.2 (bundled runtime via
`runtime-manifest.json`, discovery precedence bundled→config→PATH→mock, 方案 A
WebUI served by the backend over `STATIC_PATH`, `open_webui` window). The
current build manages the local backend lifecycle against the bundled
dev-stand-in runtime and opens its served WebUI.

Still ahead (phases F2.3–F5): swapping the dev stand-in for the real release
binary, auto-update, and one-click deployment — done after the bundled-manage
story is proven from the desktop.

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