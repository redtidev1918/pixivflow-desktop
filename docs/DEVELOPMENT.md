# Development

This guide covers building and working on PixivFlow Desktop locally. The
desktop is a **Tauri 2 + Rust** shell around two upstream components
([PixivFlow](https://github.com/redtidev1918/PixivFlow) backend and
[pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui)).

## Prerequisites

- **Rust toolchain** — install via [rustup](https://rustup.rs). The project
  respects a `rust-toolchain.toml` if you add one; `Cargo.toml` pins
  `rust-version = "1.77.2"`.
- **Node.js 20+** (Node 22.13+ is required to *run* the bundled PixivFlow
  backend; the desktop build scripts only need npm).
- **Tauri 2 system dependencies** — see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/):

  - **Windows 10/11**: WebView2 runtime, MSVC build tools, NSIS (for installer).
  - **macOS**: Xcode Command Line Tools; Apple Silicon or Intel are both targets.
  - **Linux**: `webkit2gtk-4.1`, GTK 3, `librsvg`, `appimage`/`linuxdeploy`
    tooling for AppImage bundling.

## Install

```bash
git clone https://github.com/redtidev1918/pixivflow-desktop.git
cd pixivflow-desktop
npm install
```

## Bundle the upstream assets

The desktop bundles the built `PixivFlow` backend (`dist/webui/index.js` +
runtime deps) and the `pixivflow-webui` static SPA (`dist/`) into
`src-tauri/resources`.

```bash
npm run bundle:webui    # copies pixivflow-webui/dist -> src-tauri/resources/webui
npm run bundle:backend  # copies PixivFlow dist + node_modules -> src-tauri/resources/backend
npm run bundle          # both, in the right order
```

By default the scripts look for the sibling repos (`../redtidev1918/PixivFlow`,
`../pixivflow-webui`). Override with `PIXIVFLOW_DIR` and `PIXIVFLOW_WEBUI_DIR`.
Bundled assets are git-ignored — they are build-time inputs, not source.

These pins are declared in [`desktop-manifest.json`](../desktop-manifest.json).

## Run in dev mode

```bash
npm run dev
```

This launches the Tauri window. It will start the local PixivFlow backend
(`node <bundled>/dist/webui/index.js`) against a loopback host, wait for
`/api/health`, and load the same-origin WebUI. Logs land under the OS
data/logs dirs (see below).

## Build installers

```bash
npm run build
```

`tauri build` produces the platform packaging (`tauri.conf.json` → `bundle.targets: "all"`):

- Windows: `PixivFlow.Setup.exe` (NSIS)
- macOS: `PixivFlow.dmg`
- Linux: `PixivFlow.AppImage`

Artifacts and their Tauri updater signature (`.sig`) are published to GitHub
Releases by CI/CI-CD (see `.github/workflows/`).

## Configuration

The desktop reads a `config.json` from the OS app-config directory (created on
first launch):

| Platform | Path (approx.) |
|---|---|
| Windows | `%APPDATA%\com.redtidev1918.pixivflow.desktop\config.json` |
| macOS   | `~/Library/Application Support/com.redtidev1918.pixivflow.desktop/config.json` |
| Linux   | `~/.config/com.redtidev1918.pixivflow.desktop/config.json` |

The shape is documented in [`desktop-config.json`](../desktop-config.json) at
the repo root (that file is a reference, not the active config). Key fields:

- `mode`: `"local"` (start a bundled backend) or `"remote"` (load an existing
  PixivFlow WebUI URL from `remoteUrl`).
- `backend.host` / `backend.port`: loopback bind (default `127.0.0.1:3000`).
- `backend.autoStart`: launch the backend on app start.
- `dataDir` / `downloadDir` / `logsDir`: empty means "use the OS app-dir
  default". PixivFlow's sqlite DB + downloads live here.

Data (DB, downloads) and logs (including the child backend's `stdout`/`stderr`)
are written under the OS app-data and logs directories respectively.

> Keep the bind host loopback unless you configure backend auth. Binding a
> non-loopback host without auth makes PixivFlow refuse to start (fail-closed).

## Icons

The master source is `assets/icon.png`. Regenerate all platform icon formats:

```bash
npm run icon   # tauri icon assets/icon.png  -> writes src-tauri/icons/
```

## Verifying your changes

- `cargo check` / `cargo test` for the Rust shell.
- `npm run bundle` then `npm run dev` for an end-to-end launch check.
- Release-grade output is built and validated in the GitHub Actions matrix
  (Windows/macOS/Linux).

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for how the pieces fit together and
[`ROADMAP.md`](ROADMAP.md) for where the project is heading.