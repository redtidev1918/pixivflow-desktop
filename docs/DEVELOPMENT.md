# Development

PixivFlow Desktop is a **Tauri 2** desktop app: a Rust core owns the system
capabilities (backend lifecycle, discovery, config, logging, native
integration), and a lightweight Vite frontend (`src/frontend`) renders the
control/status shell. The product WebUI is served by the backend over
`STATIC_PATH` (方案 A), not duplicated here.

## Prerequisites

Required:

- **Node.js** >= 22
- **Rust** (stable toolchain — `rustc`/`cargo`)
- **Tauri CLI** (`@tauri-apps/cli`, installed via `npm`)

For macOS / Windows / Linux setup see the official
[Tauri prerequisites](https://tauri.app/start/prerequisites/).

## Install

```bash
npm install          # installs Vite + @tauri-apps/* (frontend + Tauri CLI)
cd src-tauri && cargo fetch   # pull Rust dependencies
```

## Common commands

```bash
npm run build        # vite build + postbuild (frontend unit tests + dist smoke check)
npm test             # frontend unit tests (node --test src/frontend/*.test.mjs)
npm run tauri dev    # dev run: Vite dev server + debug Tauri window
cd src-tauri && cargo build   # build the Rust core only
cd src-tauri && cargo test    # headless backend lifecycle integration tests
cargo tauri build    # production bundle (Phase 4 currently inactive: bundle.active=false)
```

## How the app runs

1. The Tauri window opens with the bootstrap control UI (`src/frontend`).
2. Rust reads `desktop-config.json` from the platform app-config dir
   (auto-creates a default).
3. `discovery.rs` resolves which backend to run:
   `bundled runtime-manifest.json` → `backend.command` → `PATH` → dev mock.
4. `BackendManager.start()` spawns it and `poll_health` probes `/api/health`.
5. The control UI shows status / port / PID / health / source / version; the
   **打开 PixivFlow** button opens the backend-served WebUI in a webview window.

## Debugging the backend

- **Health:** the backend answers `GET /api/health` on the configured port
  (default `3000`). Confirm with `curl http://127.0.0.1:3000/api/health`.
- **WebUI static:** the backend serves the bundled WebUI dist over `STATIC_PATH`
  — `curl http://127.0.0.1:3000/` should return the WebUI `index.html`.
- **Logs:** the desktop app appends to `logs/desktop.log` (startup, discovery
  resolution, lifecycle events); the control UI has an **Open Logs** button.
- **Doctor:** `backend_doctor` IPC reports `source` / `version` / `port` /
  `running` / `healthy` — surfaced in the control UI footer.

## Mock backend

The bundled mock (`src-tauri/resources/mock-backend.mjs`) is the **dev/test
health stand-in** — it only answers `/api/health` (and `--version`), never any
Pixiv business logic. It is the lowest discovery priority (test only).

To run it standalone:

```bash
node src-tauri/resources/mock-backend.mjs   # serves :3000 (PORT env overrides)
```

The bundled runtime dev stand-in (`src-tauri/resources/runtime/pixivflow/dev-backend.mjs`)
proves the F2.2 bundled contract — `--version`, `/api/health`, and static WebUI
serving over `STATIC_PATH`. It is the committed default so a fresh clone runs
light (no heavy artifact required).

## Bundling the real PixivFlow runtime (F2.3)

To lay the real backend into the bundle (a git-ignored local artifact) and point
the manifest at it:

```bash
# from a local PixivFlow checkout
node scripts/fetch-pixivflow-runtime.mjs --source /path/to/PixivFlow
# or from npm (best-effort; the backend's npm-workspace deps must be published)
node scripts/fetch-pixivflow-runtime.mjs --source pixivflow@2.46.0
```

This builds/copies `dist/` + prod-only `node_modules/` + `package.json` into
`resources/runtime/pixivflow/`, materializes npm-workspace deps, writes `VERSION`,
and rewrites `runtime-manifest.json` to `["node","./dist/webui/index.js"]`.
Restore the committed dev-stand-in default (e.g. before a clean commit) with:

```bash
git checkout -- src-tauri/resources/runtime/pixivflow/runtime-manifest.json \
              src-tauri/resources/runtime/pixivflow/VERSION
```