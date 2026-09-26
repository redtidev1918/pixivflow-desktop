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

1. The app starts with **no window on screen**: the `main` control panel
   (`src/frontend`, the Vite shell) is created hidden — `"visible": false` in
   `tauri.conf.json`. It is a fallback, not the front door.
2. Rust reads `desktop-config.json` from the platform app-config dir
   (auto-creates a default).
3. `discovery.rs` resolves which backend to run:
   `bundled runtime-manifest.json` → `backend.command` → `PATH` → dev mock.
4. `BackendManager.start()` spawns it and `poll_health` probes `/api/health`.
5. As soon as the first health probe succeeds the backend-served WebUI opens
   **automatically** in its own `webui` window — that window *is* the app.
   Closing it stops the backend and quits. The menu bar carries the host
   actions instead of the panel: *Open manager*, *Open logs*, *Collect
   diagnostics*, *Cancel sign-in*. Closing the manager panel while the WebUI is
   open merely hides it. If health never arrives, the setup thread shows the
   panel (the *Start backend* button and the doctor report live there).
6. Signing in to Pixiv stays **inside the WebUI window**: the WebUI asks the
   host through `window.pixivflowHost.openLoginWindow(...)` and the host embeds
   the Pixiv authorize page as a child webview covering the window
   (the `open_login_window` command). No second window, no browser. The overlay has no
   chrome, so end it with the menu's *Cancel sign-in* (or wait out the 300 s
   timeout); on success the `code` resolves back into the WebUI's pending
   promise.
7. If a previous run crashed or was force-quit, its backend is still holding the
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
- **Locale:** the desktop UI (menu bar, dialogs, fallback panel) follows the system language (`zh` / `en`). Force
  one with `PIXIVFLOW_DESKTOP_LOCALE=zh` (or `en`) — useful when the OS language
  is not what you want to test. The WebUI has its own locale setting.
- **Diagnostics:** the fallback panel's **收集诊断 / Collect diagnostics** button (also the app menu) (IPC
  `export_diagnostics`) writes `<log dir>/diagnostics-<utc-ts>/` with the log and
  its rotations, `last-run.json`, any collected `crash-*.ips`, `doctor.json`, a
  secret-redacted `config.json` and `env.txt`, then reveals the folder in Finder.
  To collect evidence from a user, ask for that folder — not for a terminal
  session.
- **Sign-in:** the embedded sign-in logs `login: sign-in view embedded (…)`,
  `login: finished: code received (length N)` or `login: finished: cancelled, no
  code`. Only the code's *length* is logged — never the code. If the overlay
  stays black, `open_login_window` answers with the backend URL it was given;
  check the log for `login: bridge invoked (auth …)` first.
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

## Building the app bundle

The bundle must be produced by the Tauri build, never assembled by hand: the
bundler is what signs every Mach-O payload (including the bundled `node`). A
copy-pasted bundle signs nothing, and macOS then kills the standalone `node` with
`SIGKILL` — the app starts, the launcher window appears, and the only symptom is
`health not confirmed within attempt budget` / `backend did not become healthy`
in the desktop log.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
npx tauri build --bundles app      # or: npm run tauri -- build --bundles app
```

Two environment traps have already cost real debugging time:

**1. `process.execPath` is not always Node.** `scripts/fetch-pixivflow-runtime.mjs`
bundles "the running standalone node" via `process.execPath`. When the script runs
*inside* an Electron/desktop host (including this project's own DSH agent
session), that path is the host's helper executable, not Node — so a ~200 KB
binary gets laid into `resources/runtime/pixivflow/node` and the app cannot start
its backend. Always confirm the result before building:

```bash
ls -la src-tauri/resources/runtime/pixivflow/node   # must be ~110 MB, and
src-tauri/resources/runtime/pixivflow/node --version  # must print v2x.y.z
```

If it is small, or exits with `Killed: 9`, re-run the fetch under a real Node
(an absolute path from a version manager is the reliable choice):

```bash
/path/to/real/node scripts/fetch-pixivflow-runtime.mjs --source /path/to/PixivFlow
```

**2. `node_modules/.bin/tauri` misparses `process.argv` under an Electron host.**
The JS wrapper inspects `process.argv[0]` to decide whether it is running under
Node/Bun/Electron; an unrecognized host executable falls through to
`args.unshift(bin)`, which turns the host path into a subcommand and fails with
`error: unrecognized subcommand '/Applications/.../DSH Desktop Helper'`. Call the
native entry point directly instead of going through the shim:

```bash
node -e "const {run,logError}=require('./node_modules/@tauri-apps/cli/index.js'); \
  run(['build','--bundles','app'],'tauri',(e,r)=>{if(e){logError(e.message);process.exit(1);}})"
```

Keep `src-tauri/resources/webui/dist` and `src-tauri/resources/runtime/pixivflow`
restored to their committed state before committing; a built `.app` is never
committed either.

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