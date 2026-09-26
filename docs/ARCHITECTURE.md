# Architecture

PixivFlow Desktop is the **official desktop runtime environment and release
layer** for the PixivFlow ecosystem. It introduces no business logic of its
own; it layers a native desktop experience (runtime management, lifecycle,
discovery, packaging, simplified install & launch) on top of the stable
upstream components.

```
PixivFlow Desktop
      |
      v
PixivFlow Backend
      |
      v
PixivFlow WebUI
```

## Why Tauri 2?

- **Small package** — native webview, ~10–20 MB installers instead of 100 MB+.
- **Low memory** — OS native web engine, no bundled Chromium.
- **Cross-platform** — Windows / macOS / Linux from one codebase.
- **Rust process management** — small, auditable binary for backend lifecycle
  (spawn / stop / restart / health check) and native integration.
- **GitHub Releases updater** — first-class signed updates straight from GitHub
  Releases, no self-hosted updater server to operate.

The Desktop Layer is deliberately thin. It does **not** contain business logic:
no scheduling, no downloads, no delivery, no media handling, no Pixiv auth.

## System architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Desktop Layer  (this repo · Tauri 2 + Rust + Vite shell)     │
│  ─ Tauri window / app lifecycle                                │
│  ─ BackendManager: spawn · stop · restart · health            │
│  ─ discovery.rs: bundled → config → PATH → mock               │
│  ─ config (desktop-config.json) · logging · native integration│
│  ─ control shell (src/frontend) — status + 打开 PixivFlow     │
└───────────────┬──────────────────────────────────────────────┘
                │ spawn / supervise / open_webui
                ▼
┌──────────────────────────────────────────────────────────────┐
│  Backend Runtime  (upstream PixivFlow)                       │
│  ─ PixivFlow business logic (schedule · download · delivery) │
│  ─ HTTP + Socket.IO API                                      │
│  ─ serves WebUI static dist over STATIC_PATH (方案 A)        │
└───────────────┬──────────────────────────────────────────────┘
                │ same-origin http://127.0.0.1:{port}/
                ▼
┌──────────────────────────────────────────────────────────────┐
│  Web UI  (upstream pixivflow-webui)                          │
│  ─ the user interface (everything the user sees & clicks)    │
└──────────────────────────────────────────────────────────────┘
```

Layer responsibilities:

- **Desktop Layer (this repo / Tauri)** — window & app lifecycle, backend
  process management, runtime discovery, configuration, logging, native
  integration. It renders the upstream UI in a webview window and never
  re-implements the interface.
- **Backend Layer (upstream PixivFlow)** — the execution plane: PixivFlow
  business logic, the WebUI-facing HTTP + Socket.IO API, the scheduler, and the
  download pipeline.
- **Web Layer (upstream pixivflow-webui)** — the UI and all user interaction.

## Runtime lifecycle

1. **Resolve** — `discovery.rs` chooses the backend command:
   bundled `runtime-manifest.json` → `backend.command` → `PATH` → dev mock.
2. **Spawn** — `BackendManager.start()` injects the resolved
   `LaunchSpec{command, env}` (with `STATIC_PATH`) and launches the process
   (idempotent; refuses if the port is already taken).
3. **Health** — periodic `GET /api/health` probe (raw TCP) drives the UI
   `running / healthy` state; `backend_doctor` surfaces source / version / port.
4. **Serve WebUI** — the backend hosts the static WebUI over `STATIC_PATH`;
   the desktop opens `http://127.0.0.1:{port}/` (方案 A).
5. **Stop** — graceful **SIGTERM** with a bounded wait; never `kill -9`.
   Closing the desktop window triggers the stop.

## Backend communication

- The desktop talks to the backend **only** as an external client over its HTTP
  API (health + WebUI). It never links, imports, or patches backend code.
- The backend is configured via **environment / CLI overrides** (`PORT`,
  `HOST`, `STATIC_PATH`) — not by modifying upstream source.
- The WebUI is **same-origin** with the backend, so the desktop window needs no
  cross-origin proxy for the product page.

## Desktop ⇄ PixivFlow boundary (non-negotiable)

- Desktop ⇢ launches / supervises / updates the backend. **Never replaces it.**
- Desktop ⇢ renders the WebUI. **Never re-implements it.**
- Desktop ⇢ reads historical analysis/docs. **Never imports upstream source.**
- **PixivFlow is the only source of business behavior**; the Desktop never
  copies backend logic.

## Component version lock

`desktop-manifest.json` declares which upstream versions the desktop expects.
`releasegraph` reads it to drive automatic upgrade PRs (see
[RELEASE.md](RELEASE.md)).

## Backend command adapter & discovery (F2.1)

`BackendManager` is deliberately a **pure lifecycle owner**: `start` / `stop` /
`restart` / `health_check`. It never learns PixivFlow business logic. Deciding
*which command to run* is the job of the discovery/adapter layer
(`src-tauri/src/backend/discovery.rs`), which resolves in priority order:

1. **bundled** `resources/runtime/pixivflow/` — described by an inline
   `runtime-manifest.json` (`{name, version, platform, command[], args[],
   health, staticPath, servesWebui}`); a legacy bare
   `pixivflow`/`pixivflow.exe` executable is accepted as a fallback.
2. **user-configured** `backend.command` + `backend.args` (user override).
3. `pixivflow` found on `PATH`.
4. fallback: bundled **mock** backend (dev/testing only).

### Resource resolution in dev vs. installed bundle (F4.0)

The paths above are *logical* resource paths. `discovery.rs` resolves each one
against, in order:

1. the installed bundle resource root — Tauri `app.path().resource_dir()`
   (e.g. `PixivFlow Desktop.app/Contents/Resources/`), injected once via
   `set_resource_root()` in the setup hook;
2. the compile-time cargo manifest dir (`CARGO_MANIFEST_DIR/resources/…`) —
   used by `cargo tauri dev` and by tests;
3. a CWD-relative dev path (`src-tauri/resources/…`).

So the same discovery code works unmodified in dev, tests and the installed
`.app` / `.AppImage`; no absolute build-machine path leaks into a shipped app.

`args[]` are appended to `command[]`; the manifest's optional `staticPath`
(relative to the runtime dir, or absolute) overrides the desktop's bundled
`resources/webui/dist` when it resolves to a directory.

Resolution result (`BackendDescriptor{source, executable_path, command,
version, static_path, serves_webui}`) is injected into the manager via
`set_command_override(Some(LaunchSpec{command, env}))` before `start()`; the
`LaunchSpec` carries the extra env (`STATIC_PATH=…`) needed for 方案 A WebUI
serving. The manager's own fallback (config `command` → mock) is unchanged. The
`backend_doctor` Tauri command surfaces the same resolution plus live runtime
status (`running` / `healthy`) **and two F2.3 reports**:

- `runtime` (`validate_bundled_runtime()`): manifest found/valid, entry file
  exists, version, declared platform vs. current host (lenient), servesWebui.
- `webui`: STATIC_PATH on-disk presence and live `GET /` accessibility while
  the backend is running and healthy.

The reported `version` is trusted from the runtime manifest when present (a
bundled release binary may not support `--version` — it would boot a server
instead); only version-less sources (PATH / config binaries) get an
`--version` probe under a timeout.

### Runtime acquisition (F2.3)

The bundled runtime is filled by **`scripts/fetch-pixivflow-runtime.mjs`** (build
from a local PixivFlow checkout, or fetch an npm spec). It lays out
`resources/runtime/pixivflow/{dist, node_modules, package.json, VERSION}` and
rewrites `runtime-manifest.json` to `command:["node"]` +
`args:["./dist/webui/index.js"]`. Those are **git-ignored build products**; the
committed default manifest still points at the lightweight dev stand-in
(`dev-backend.mjs`), so a fresh clone runs without the heavy artifact until the
fetch runs. npm-workspace packages the built dist depends on (e.g.
`@redtidev/pixiv-client`) are materialized into the runtime's `node_modules` so
the bundle is self-contained.

### Release contract (future — design only)

A future release bundle adds `webui/` and `checksums.json` beside the runtime
(`{relpath: sha256-hex}`), fetched/verified at install or into
`~/.pixivflow/runtime/` for upgrades. **Not implemented**; desktop currently
bundles via the fetch script only, and no auto-update exists.

### WebUI integration (方案 A) — F2.2

The desktop does **not** maintain a second frontend. The resolved backend is
expected to serve the static WebUI over `STATIC_PATH` (the bundled
`resources/webui/dist`), and the `open_webui` Tauri command opens
`http://127.0.0.1:{port}/` in a dedicated `webui` webview window (reusing it if
already open). The Tauri shell (Vite `src/frontend`) is only the *control /
status* surface, separate from the WebUI product page.

Backend config shape (flat, per the desktop-config contract — `mode` is a
**top-level** field, not under `backend`):

```json
{ "mode": "local", "backend": { "port": 3000, "autoStart": true, "command": "", "args": [] } }
```