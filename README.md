# PixivFlow Desktop

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)]()
[![Made with Tauri 2](https://img.shields.io/badge/Made%20with-Tauri%202-purple)]()

> A cross-platform desktop shell for the **PixivFlow** ecosystem. It bundles the
> [PixivFlow](https://github.com/redtidev1918/PixivFlow) backend and its
> [pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) front end
> into a single, installable desktop app for ordinary users.

**Design principle:** PixivFlow Desktop is **not a rewrite** of PixivFlow. It is
a thin *launcher and container* — it starts the backend, opens the WebUI, manages
the child process, and creates a download-and-run experience. It never copies
upstream business logic and never modifies the upstream repositories.

## Why Tauri 2?

| | Tauri 2 | Electron (alternative) |
|---|---|---|
| Installer size | ~10–20 MB | 100 MB+ |
| Memory footprint | Low (native webview) | High (bundled Chromium) |
| Web engine | OS-native (WebView2 / WKWebView / webkit2gtk) | Bundled Chromium |
| Update system | First-class (signed, via GitHub Releases) | Requires self-hosted or third-party service |
| Security model | Capability-based permissions | Chromium sandbox + rebuilds |

Tauri is the right fit for a process-managing shell: the **Rust** side handles
backend lifecycle, health checks, and graceful shutdown with a small, auditable
binary — and updates flow straight from GitHub Releases (no separate updater
server to operate).

## Relationship to the ecosystem

```
┌─────────────── releasegraph ───────────────┐
│   orchestrates versions + release pipelines  │
└───────────────────────────────┬─────────────┘
                                │ drives
┌───────────────────────────────┴─────────────────┐
│                 PixivFlow Desktop                │
│                       Tauri 2                    │
│  ┌───────────────────────────────────────────┐   │
│  │            pixivflow-webui                │   │  (same-origin, embedded)
│  └──────────────────┬────────────────────────┘   │
│                     │ 127.0.0.1:<port>            │
│  ┌──────────────────┴────────────────────────┐   │
│  │           PixivFlow Backend               │   │  (child process managed
│  │       /api + /socket.io + static UI       │   │   by BackendManager)
│  └───────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

- **PixivFlow (backend)** — the source of truth for downloads*, config, DB &
  API (Node.js ≥ 22.13, Express 4 + Socket.IO, self-bootstrapped SQLite).
- **pixivflow-webui** — React 18 + Vite SPA; packaged once and served
  *same-origin* by the backend via `STATIC_PATH`.
- **releasegraph** — the fleet control-plane (`fleet.yaml` / `.release-policy.yml`)
  that versions each repo and — with this repo — will drive the desktop's
  automatic upgrades.

### Why this integration "just works" without touching upstream

The desktop launches the backend with desktop-derived env overrides, then loads
the same-origin URL in the main window:

```
env: PORT / HOST=127.0.0.1 / PIXIV_DATABASE_PATH / PIXIV_DOWNLOAD_DIR /
     STATIC_PATH=<bundled webui dist> / PIXIV_LOG_LEVEL
→  backend serves  static UI + /api + /socket.io  on the same port
→  relative /api and /socket.io work with ZERO CORS and ZERO webui changes
→  backend SPA fallback keeps deep links (/dashboard, /login) refreshes working
```

## Supported platforms

| Platform | Installer |
|---|---|
| Windows 10/11 | `PixivFlow.Setup.exe` |
| macOS — Intel & Apple Silicon | `PixivFlow.dmg` |
| Linux | `PixivFlow.AppImage` |

## Quick start (development)

```bash
git clone https://github.com/redtidev1918/pixivflow-desktop.git
cd pixivflow-desktop

npm install                  # @tauri-apps/cli

# bundle the two upstream repos into resources (paths overridable via
# $PIXIVFLOW_DIR and $PIXIVFLOW_WEBUI_DIR):
npm run bundle

npm run dev                  # builds Rust shell + launches the app
```

`npm run dev` will compile the Rust shell, let `BackendManager` start a local
PixivFlow backend, wait for `/api/health`, then open the WebUI at
`http://127.0.0.1:3000`.

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for the full prerequisite matrix
and how to build the installers.

## Configuration

The desktop reads a `config.json` from the OS app-config directory (created on
first launch). The documented shape lives in
[`desktop-config.json`](desktop-config.json). Two modes:

| Field | Meaning |
|---|---|
| `mode: "local"` (default) | Desktop starts a bundled backend and loads its WebUI |
| `mode: "remote"` + `remoteUrl` | Desktop connects to an *existing* PixivFlow server (keeps Docker/host-deployed users on the desktop with zero migration) |

Data (sqlite DB, downloads) and logs live under the OS app-data/logs dirs; the
bound host defaults to loopback (`127.0.0.1`) so the backend stays
credential-free unless you explicitly configure auth.

## Component version lock (`desktop-manifest.json`)

[`desktop-manifest.json`](desktop-manifest.json) pins the exact upstream versions
the desktop bundles and ships. It is the input for the releasegraph-driven
**auto-upgrade** flow: when an upstream releases, releasegraph detects the stale
pin, opens a merge-queue upgrade PR, and — once the build gate passes and it
merges — this repo's own release pipeline publishes fresh installers + the Tauri
updater payload. No manual "find the newest build" bookkeeping.

## Roadmap

**F0** Foundation (structure/docs/contracts) ✅ · **F1** Shell MVP ·
**F2** Local integration & robustness · **F3** Configuration & remote mode ·
**F4** Distribution (signing/notarization) · **F5** Automatic updates ·
**F6** releasegraph integration · **F7** v1.0 polish

Full plan: [docs/ROADMAP.md](docs/ROADMAP.md).

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — how the desktop and upstreams connect
- [Development](docs/DEVELOPMENT.md) — building, configuring, packaging
- [Release ecosystem analysis](docs/RELEASE-ECOSYSTEM-ANALYSIS.md) — the
  multi-repo auto-release design

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues → [SECURITY.md](SECURITY.md).
Code of conduct → [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

MIT © [redtidev1918](https://github.com/redtidev1918). The upstream
[PixivFlow](https://github.com/redtidev1918/PixivFlow) and
[pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) each carry
their own licenses — see their repositories.