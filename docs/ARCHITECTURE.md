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
│  ─ Tauri window / app lifecycle · app menu, embedded sign-in  │
│  ─ BackendManager: spawn · stop · restart · health · adopt     │
│  ─ discovery.rs: bundled → config → PATH → mock               │
│  ─ config (desktop-config.json) · logging · native integration│
│  ─ fallback panel (src/frontend) — only on start failure      │
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

### Window model (F4.1)

Two windows exist and normally only one is visible:

- **`webui` — the app.** Created on `http://127.0.0.1:{port}/` with the host
  bridge injected, shown as soon as health passes. Closing it stops the backend
  and quits the process. The WebUI window is *not* a child of the launcher: it
  is the product surface, and the launcher is no longer the "first" window.
- **`main` — the fallback panel** (`"visible": false` in `tauri.conf.json`). The
  Vite shell in `src/frontend` is backend status + controls, shown only when the
  backend never becomes healthy, or on demand from the menu bar. Closing it
  while `webui` is open just hides it.

Because the panel is normally invisible, the app menu
(`build_menu()` in `src-tauri/src/lib.rs`) carries the host-level actions: *Open
manager*, *Open logs*, *Collect diagnostics*, *Cancel sign-in*.
- **Backend Layer (upstream PixivFlow)** — the execution plane: PixivFlow
  business logic, the WebUI-facing HTTP + Socket.IO API, the scheduler, and the
  download pipeline.
- **Web Layer (upstream pixivflow-webui)** — the UI and all user interaction.

## Runtime lifecycle

1. **Resolve** — `discovery.rs` chooses the backend command:
   bundled `runtime-manifest.json` → `backend.command` → `PATH` → dev mock.
2. **Adopt or spawn** — if a healthy PixivFlow runtime from a previous run is
   still listening on the port (force-quit / crash), `start()` **adopts** it
   instead of failing (see *Backend ownership* below). Otherwise it injects the
   resolved `LaunchSpec{command, args, env, cwd}` (with `STATIC_PATH`) and
   launches the process (idempotent; refuses if the port is held by something
   that is not a PixivFlow runtime).
3. **Health** — periodic `GET /api/health` probe (raw TCP) drives the UI
   `running / healthy` state; `backend_doctor` surfaces source / version / port.
4. **Serve WebUI** — the backend hosts the static WebUI over `STATIC_PATH`;
   the desktop opens `http://127.0.0.1:{port}/` (方案 A) **automatically** as
   soon as the first health probe succeeds — the user never has to click
   "打开 PixivFlow".
5. **Stop** — graceful **SIGTERM** with a bounded wait; never `kill -9`.
   Closing the **WebUI window** stops the backend (spawned *or* adopted) and
   quits; the hidden `main` panel only hides while the WebUI is open. Quitting
   the app (Cmd+Q, the app menu, or `osascript quit`) stops it too: a macOS quit
   never reaches the window close handler, so `lib.rs` also handles
   `RunEvent::ExitRequested` / `RunEvent::Exit` with the same idempotent
   `stop()`.

## Backend communication

- The desktop talks to the backend **only** as an external client over its HTTP
  API (health + WebUI). It never links, imports, or patches backend code.
- The backend is configured via **environment / CLI overrides** (`PORT`,
  `HOST`, `STATIC_PATH`) — not by modifying upstream source.
- The backend's **working directory is the per-user data root** (see below), so
  PixivFlow's own relative defaults (`./data`, `./downloads`, `config/`) land
  under the user's application-support directory instead of a random CWD. This
  is deliberate: PixivFlow's path auto-fixer rejects absolute paths outside its
  `process.cwd()`, so injecting *absolute* storage paths via env would be
  rewritten back to `./data`.
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
`set_command_override(Some(LaunchSpec{command, args, env, cwd}))` before
`start()`; the `LaunchSpec` carries the extra env (`STATIC_PATH=…`) needed for
方案 A WebUI serving and the per-user data root as `cwd`. The manager's own
fallback (config `command` → mock) is unchanged. The
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
`resources/runtime/pixivflow/{dist, node_modules, package.json, VERSION, node}`
and rewrites `runtime-manifest.json` to `command:["./node"]` +
`args:["./dist/webui/index.js"]`. `node` is a **standalone Node binary copied
into the runtime directory**, so the app carries its own interpreter: no `PATH`
lookup, no dependency on a system Node install, and no launcher shim that
re-parents the backend when the app dies. Those are **git-ignored build
products**; the
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

The WebUI window is opened **automatically** by the setup hook once health has
been confirmed (`poll_health_for` → `open_webui_window` on the main thread). The
`main` panel stays hidden; if health never arrives, the setup thread's failure
branch shows it instead (the user still gets a window with a *Start backend*
button and the doctor report).

Closing the **WebUI** window stops the backend and quits. Closing `main` while a
`webui` window exists only hides it.

### Embedded Pixiv sign-in (F4.1)

The WebUI's sign-in flow asks the host to show the Pixiv authorize page through
`window.pixivflowHost.openLoginWindow(authUrl, redirectUri)` (bridge injected by
`commands::HOST_BRIDGE_SCRIPT`), which invokes the Rust command
`open_login_window`. The host renders that page as a **child webview** covering
the `webui` window (`Window::add_child`, label `login`) instead of opening a
second OS window or the user's browser:

- the WebUI page keeps running underneath, so the pending
  `openLoginWindow()` promise still resolves with `{ code }` — the WebUI and the
  backend contract are untouched;
- `on_navigation` intercepts the `…/web/v1/users/auth/pixiv/callback`
  redirect, extracts `code`, and blocks the navigation (only `scheme://host/path`
  is ever logged, never the query);
- the child has no chrome, so the host owns cancellation: the app menu's
  *Cancel sign-in* → `commands::cancel_login()` resolves `None`, as does the
  300 s timeout;
- the remote Pixiv origin holds **no** capability — Tauri's ACL refuses every
  `invoke` from it by design;
- `add_child` is gated behind tauri's `unstable` feature (multi-webview), which
  is why `src-tauri/Cargo.toml` enables it. The overlay is resized with its
  parent from the `RunEvent::WindowEvent { Resized }` hook and always closed
  before the command returns.

Backend config shape (flat, per the desktop-config contract — `mode` is a
**top-level** field, not under `backend`):

```json
{ "mode": "local", "backend": { "port": 3000, "autoStart": true, "command": "", "args": [] } }
```

### Backend ownership & adoption (F4.1)

`BackendManager` distinguishes the backend it **spawned** (owns the `Child`) from
one it **adopted**. Adoption exists because a force-quit or a crash does not run
the stop path: the backend is re-parented to `launchd`/`init` and keeps holding
the port, so the next launch used to fail with "端口 3000 已被占用".
`start()` therefore tries `adopt_existing_backend()` first, which requires:

- the port is open, and
- `GET /api/health` answers `200`, and
- the listening pid's command line contains `pixivflow` (so an unrelated port
  occupant is still a hard error).

An adopted backend counts as `running`, reports its pid, and `stop()` SIGTERMs it
by pid. `backend_doctor` / the launcher log distinguish `started` from `adopted`.

### Per-user data root (F4.1)

The setup hook resolves `app.path().app_local_data_dir()/pixivflow` (macOS:
`~/Library/Application Support/dev.redtidev.pixivflowdesktop/pixivflow/`),
creates it, and stores it as the process CWD for the backend. PixivFlow then
creates its own `config/`, `data/` (SQLite) and `downloads/` there. Nothing is
written next to the installed `.app`.

## Observability

Everything the app knows about its own run ends up in one directory, so a report
from a user is a folder rather than a terminal session. All of it is
dependency-free (`src-tauri/src/logger.rs`).

- **Logger** — a `BufWriter<File>` behind a `Mutex`, appending
  `2026-09-26T18:21:48Z [LEVEL] message` lines and mirroring them to stdout for
  `cargo tauri dev`. The path is the OS log dir
  (`app_log_dir()` → `~/Library/Logs/dev.redtidev.pixivflowdesktop/`), else
  `logDir` from the config, else a CWD-relative `logs/`. At startup an oversized
  `desktop.log` is rotated at 2 MiB into three generations.
- **Session marker** — the first lines record version, OS/arch, pid, the resolved
  log / config / data / resource paths, the locale and whether a system proxy was
  injected into the backend environment.
- **Panic hook** — installed before any window or backend work. A panic appends
  `PANIC <payload> at <file>:<line>` plus a forced backtrace through a **fresh
  file handle** (never the logger mutex, which the panicking thread may hold) and
  also writes a sibling `panic-<utc-ts>.log`, then chains to the default hook so
  stderr keeps its message.
- **Run-event trace** — `app.run`'s callback logs every `RunEvent` compactly, so
  the log shows why the app exited; per-click focus / scale / move events are
  filtered out, window events carry their label.
- **Crash evidence** — `last-run.json` (`{startedAt, cleanExit, pid, version}`)
  is written at startup and rewritten on exit. An unclean previous run logs a
  WARN and copies the newest matching `pixivflow-desktop-*.ips` from
  `~/Library/Logs/DiagnosticReports/` into the log dir as `crash-<ts>.ips` —
  macOS retires and purges the originals, so the copy is the durable evidence.
- **Frontend forwarding** — the injected host bridge and `src/frontend/main.js`
  forward `error` / `unhandledrejection` (and launcher render errors) to the
  `log_frontend` command, capped per page load and truncated, so a WebUI-side
  failure still leaves a Rust-side trace. The remote `webui` capability allows
  exactly two commands: the login bridge and `log_frontend`.
- **Diagnostics bundle** — `export_diagnostics` copies the log + rotations,
  `last-run.json`, collected `crash-*.ips`, `doctor.json`, a `config.json` whose
  secret-looking values are replaced by `"***"`, and `env.txt` into
  `<log dir>/diagnostics-<utc-ts>/`, then reveals it in the OS file manager.
