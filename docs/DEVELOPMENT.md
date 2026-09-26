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
cargo tauri build                 # production build + installer bundles (DMG/…/AppImage)
cargo tauri build --no-bundle     # release binary only
cargo tauri build --bundles app   # macOS .app only (fastest packaging check)
```

> Under the DSH harness the Tauri CLI can mistake the Electron helper path for
> an argument; if so invoke it directly:
> `node -e "require('./node_modules/@tauri-apps/cli/main.js').run(['icon','src-tauri/icons/icon.png'],'cargo-tauri')"`
> (cargo must be on `PATH`, e.g. `PATH="$HOME/.cargo/bin:$PATH"`).

## How the app runs

1. The Tauri window opens with the bootstrap control UI (`src/frontend`).
2. Rust reads `desktop-config.json` from the platform app-config dir
   (auto-creates a default).
3. `discovery.rs` resolves which backend to run:
   `bundled runtime-manifest.json` → `backend.command` → `PATH` → dev mock.
4. `BackendManager.start()` spawns it and `poll_health` probes `/api/health`.
5. As soon as the first health probe succeeds the backend-served WebUI opens
   **automatically** in its own webview window; the launcher stays behind it.
   Closing the WebUI window returns to the launcher and keeps the backend
   running — the **打开 PixivFlow** button re-opens it. Closing the *launcher*
   stops the backend and closes the WebUI window.
6. If a previous run crashed or was force-quit, its backend is still holding the
   port: the next launch **adopts** that process instead of failing. The log says
   `auto-start: backend adopted pid=…` instead of `started`.

## Debugging the backend

- **Data root:** the backend runs with its CWD set to
  `~/Library/Application Support/dev.redtidev.pixivflowdesktop/pixivflow/`
  (macOS; `app_local_data_dir()/pixivflow` elsewhere). PixivFlow's `config/`,
  `data/` and `downloads/` live there — **not** next to the repo or the `.app`.
- **Health:** the backend answers `GET /api/health` on the configured port
  (default `3000`). Confirm with `curl http://127.0.0.1:3000/api/health`.
- **Port stuck after a crash:** the app adopts the surviving backend on the next
  launch, so a stuck port is usually harmless. To clear it manually:
  `lsof -nP -iTCP:3000 -sTCP:LISTEN` then `kill <pid>`.
- **WebUI static:** the backend serves the bundled WebUI dist over `STATIC_PATH`
  — `curl http://127.0.0.1:3000/` should return the WebUI `index.html`.
- **Logs:** the desktop app appends to the **OS log dir** —
  `~/Library/Logs/dev.redtidev.pixivflowdesktop/desktop.log` on macOS — not to the
  repo. A non-empty `logDir` in `desktop-config.json` overrides it; a
  CWD-relative `logs/desktop.log` is only the last resort. It rotates at 2 MiB,
  keeping `desktop.log.1`…`.3`, and an unclean exit is recorded in
  `last-run.json` in the same dir. The control UI has an **Open Logs** button.
- **Locale:** the launcher UI follows the system language (`zh` / `en`). Force
  one with `PIXIVFLOW_DESKTOP_LOCALE=zh` (or `en`) — useful when the OS language
  is not what you want to test. The WebUI has its own locale setting.
- **Diagnostics:** the launcher's **收集诊断 / Collect diagnostics** button (IPC
  `export_diagnostics`) writes `<log dir>/diagnostics-<utc-ts>/` with the log and
  its rotations, `last-run.json`, any collected `crash-*.ips`, `doctor.json`, a
  secret-redacted `config.json` and `env.txt`, then reveals the folder in Finder.
  To collect evidence from a user, ask for that folder — not for a terminal
  session.
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

## Bundling the real WebUI (F4.1)

The committed `src-tauri/resources/webui/dist/index.html` is only a placeholder
— the real UI comes from the `pixivflow-webui` repo and stays a git-ignored
build product:

```bash
cd ../pixivflow-webui && npm ci && npm run build
rm -rf ../pixivflow-desktop/src-tauri/resources/webui/dist
cp -R dist ../pixivflow-desktop/src-tauri/resources/webui/dist
```

The backend serves that directory over `STATIC_PATH`. Restore the committed
placeholder with
`git checkout -- src-tauri/resources/webui/dist/index.html` and remove
`src-tauri/resources/webui/dist/assets/`.

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
`resources/runtime/pixivflow/`, materializes npm-workspace deps, copies a
**standalone `node` binary** next to them (so the app needs no system Node and
no launcher shim that would re-parent the backend), writes `VERSION`, and
rewrites `runtime-manifest.json` to the formal contract: `command:["./node"]` +
`args:["./dist/webui/index.js"]`, plus `version`, `platform`, `health`,
`servesWebui`. The doctor validates this bundle
(manifest/entry/platform) and reports WebUI presence + live accessibility.
Restore the committed dev-stand-in default (e.g. before a clean commit) with:

```bash
git checkout -- src-tauri/resources/runtime/pixivflow/runtime-manifest.json \
              src-tauri/resources/runtime/pixivflow/VERSION
```

The real backend writes a default `config/standalone.config.json` into its CWD
on first boot. Launched by the app that is the per-user data root above; if you
run the runtime **manually from this repo**, it lands in the repo and a stray
`config/` dir is safe to remove. Future releases may add `checksums.json` verification and a
download/upgrade flow — that is design-only today.