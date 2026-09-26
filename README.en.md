# PixivFlow Desktop

<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="PixivFlow Desktop" width="128" />
</p>

**Language / 语言:** [中文](README.md) · English

> **The native desktop build of PixivFlow, for Windows / macOS / Linux.**

PixivFlow Desktop is the official desktop distribution of [PixivFlow](https://github.com/redtidev1918/PixivFlow): it packages the PixivFlow runtime together with its WebUI into a native app you just double-click. Nothing else has to be installed — no Node, no server to start. The desktop layer only launches, supervises and bundles the local runtime; **all business logic stays in upstream PixivFlow**.

[Full documentation](https://redtidev1918.github.io/pixivflow-desktop/#/en/)

[![GitHub license](https://img.shields.io/github/license/redtidev1918/pixivflow-desktop?style=flat)](LICENSE) [![GitHub release](https://img.shields.io/github/v/release/redtidev1918/pixivflow-desktop?style=flat)](https://github.com/redtidev1918/pixivflow-desktop/releases) [![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat)](https://github.com/redtidev1918/pixivflow-desktop/releases) [![Made with Tauri 2](https://img.shields.io/badge/Made%20with-Tauri%202-purple?style=flat)](https://tauri.app/) [![Docs](https://img.shields.io/badge/Docs-documentation%20site-6366f1?style=flat)](https://redtidev1918.github.io/pixivflow-desktop/#/en/)

## Download

Grab your platform's installer from [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) (latest version and checksums on the [download page](https://redtidev1918.github.io/pixivflow-desktop/#/en/download.md)):

| Platform | File | First launch |
| :-- | :-- | :-- |
| **macOS** (Apple Silicon) | `PixivFlow-Desktop-v<version>-macos-arm64.dmg` | Unsigned: **right-click → Open** in Finder, or allow it under System Settings → Privacy & Security |
| **Windows** 10/11 | `PixivFlow-Desktop-v<version>-windows-x64-setup.exe` | Unsigned: SmartScreen → *More info* → *Run anyway* |
| **Linux** (x64) | `PixivFlow-Desktop-v<version>-linux-amd64.deb` | Launch directly |

Every release also ships `SHA256SUMS` and `RELEASE-METADATA.json`:

```bash
sha256sum -c SHA256SUMS
```

The installers are **not code-signed**, so the OS will block them once — a known limitation, not a broken download. macOS currently produces an Apple Silicon `.dmg` only (why: see [release & version management](docs/RELEASE.md)).

## Features

- **Double-click and go** — the app reads its config, starts the bundled PixivFlow backend, waits for the health check and opens the WebUI. It carries its own Node runtime, so **the user machine needs nothing installed**.
- **The window *is* the WebUI** — the app opens the PixivFlow interface directly; the control panel only appears when the backend cannot start (the menu bar always offers *Open manager* / *Open logs* / *Collect diagnostics* / *Cancel sign-in*).
- **Sign-in inside the app** — the Pixiv authorize page opens as an overlay inside the WebUI window: no second window, no external browser, and the result goes straight back to the WebUI.
- **Local backend lifecycle** — the Rust `BackendManager` starts, **adopts** (a backend left behind by a crash), gracefully stops (on window close *and* app quit) and restarts the backend, and discovers the runtime in the order **bundle → config → PATH → dev mock**.
- **Bundled runtime** — the installer carries the full PixivFlow backend (`resources/runtime/pixivflow/`, described by `runtime-manifest.json`) plus the WebUI dist, so ordinary users install nothing else.
- **Visible runtime status** — live port / PID / health readout fed by the `/api/health` probe and pushed to the UI through the `backend-status` event.
- **Config and data placement** — `desktop-config.json` (created on first run) controls the backend port, auto-start and command override; your PixivFlow data (`config/`, `data/`, `downloads/`) lives in the **per-user app data directory**, not next to the app.
- **System-language UI + one-click diagnostics** — the menu bar, dialogs and fallback panel follow the system language (Chinese / English), and **Collect diagnostics** bundles logs, the last-run record, any native crash report, doctor output and a secret-free config into one folder it reveals in your file manager — the supported way to send a support report.

## Quick start

### Installer

1. Download your platform's installer from [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) and install it (allow it on first launch as described above).
2. Start **PixivFlow Desktop**: it brings up the local backend and opens the WebUI.
3. Sign in to Pixiv once inside the window; configuration and data stay on your machine.

### From source

You need Node 20+, a Rust toolchain and the [Tauri prerequisites](https://tauri.app/start/prerequisites/):

```bash
npm install              # Vite + @tauri-apps/* (frontend + Tauri CLI)
cd src-tauri && cargo fetch && cd ..

npm run tauri dev        # dev run: Vite dev server + debug window
npm test                 # frontend unit tests + version consistency + dist smoke check
cd src-tauri && cargo test   # backend lifecycle integration tests
```

Lay the **real** PixivFlow runtime into the bundle (a git-ignored build product):

```bash
npm run runtime:fetch    # defaults to the sibling ../redtidev1918/PixivFlow
npm run webui:fetch      # compose the real WebUI into src-tauri/resources/webui/dist
```

The committed default runtime is the light `dev-backend.mjs` stand-in, so a fresh clone runs without either step. More in the [development guide](docs/DEVELOPMENT.md).

## Architecture

```
PixivFlow Desktop (Tauri 2 + Rust)
        |
        v
PixivFlow backend (upstream — all business logic)
        |
        v
PixivFlow WebUI (upstream pixivflow-webui, opened directly)
```

Three layers, one responsibility each: the **Desktop** (this repo) packages, launches, supervises and diagnoses the runtime; the **Backend** (upstream PixivFlow) owns all business logic; the **WebUI** (upstream pixivflow-webui) is the product surface the desktop opens. The desktop never duplicates the frontend and never implements business rules. See [architecture](docs/ARCHITECTURE.md).

## Why Tauri 2

- **Small package** — the OS-native WebView keeps installers in the tens of megabytes instead of a Chromium-sized download.
- **Low memory** — it reuses the web engine the OS already ships.
- **One codebase, three platforms** — the same Rust + frontend code on Windows, macOS and Linux.
- **Rust owns the process** — start / stop / health-check logic lives in a small, auditable native binary.
- **Release pipeline included** — installers land directly as GitHub Release assets, built and validated by the fleet engine.

## Relationship to the ecosystem

```
PixivFlow  ──►  pixivflow-webui  ──►  pixivflow-desktop
                        │
                        └── releasegraph manages versions and releases
```

Upstream is [PixivFlow](https://github.com/redtidev1918/PixivFlow) (runtime and business logic)
and [pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) (browser UI); this
repository only packages them into a native app and manages launch, supervision and upgrades.

[releasegraph](https://github.com/redtidev1918/releasegraph) drives version relationships and release pipelines across the ecosystem. This repo's [`desktop-manifest.json`](desktop-manifest.json) is the **component version lock** (currently one version each for `pixivflow` and `pixivflow-webui`) that the build reads to acquire upstream artifacts — and that will drive the desktop's automatic upgrades. See [relationship](docs/RELATIONSHIP.md).

## Development

| Command | What it does |
| :-- | :-- |
| `npm run tauri dev` | Run the desktop app in development |
| `npm run build` | Build the frontend + `postbuild` smoke check |
| `npm test` | Frontend unit tests + version consistency + dist check |
| `npm run test:version` | Verify the three version files agree (`package.json` / `tauri.conf.json` / `Cargo.toml`) |
| `cd src-tauri && cargo test` | Backend lifecycle integration tests |
| `bash scripts/build-release` | Run one release build locally into `dist/release/` |

Read [AGENTS.md](AGENTS.md) before changing code: it states this repo's boundaries (no business logic, no second frontend, no auto-update) plus the commit and release conventions.

## Known limits

- The installers are **unsigned**; first launch needs a manual allow (see [Download](#download)).
- macOS currently ships **Apple Silicon (arm64)** only: `macos-latest` is itself Apple Silicon and the bundled `node` is the runner's own. An Intel build needs the runtime distributed as per-platform release artifacts first ([roadmap](docs/ROADMAP.md) F4.2 / F4.3).
- There is no auto-update yet ([roadmap](docs/ROADMAP.md) F5).

## Versioning & releases

Versioning rules, the release flow and the installer naming contract live in [release & version management](docs/RELEASE.md); the current phase and what comes next are in the [roadmap](docs/ROADMAP.md).

## Documentation

The repository docs *are* the source of the [documentation site](https://redtidev1918.github.io/pixivflow-desktop/#/en/):

| Document | Contents |
| :-- | :-- |
| [Quick start](docs/QUICKSTART.md) | Install, first launch, sign-in, common actions |
| [Architecture](docs/ARCHITECTURE.md) | Layer boundaries, backend lifecycle, runtime discovery, 方案 A |
| [Development](docs/DEVELOPMENT.md) | Prerequisites, commands, layout, debugging |
| [Release & versions](docs/RELEASE.md) | Release flow, three-file version consistency, asset contract, limits |
| [Roadmap](docs/ROADMAP.md) | Phases F0 → F5 with acceptance criteria |
| [Relationship](docs/RELATIONSHIP.md) | Dependencies and version lock with PixivFlow / pixivflow-webui / releasegraph |
| [Troubleshooting](docs/TROUBLESHOOTING.md) | Startup failures, busy ports, white screen, sign-in issues, logs and diagnostics |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues → [SECURITY.md](SECURITY.md), community guidelines → [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

MIT © [redtidev1918](https://github.com/redtidev1918). Upstream [PixivFlow](https://github.com/redtidev1918/PixivFlow) and [pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) carry their own licenses.
