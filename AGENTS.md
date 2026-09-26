# PixivFlow Desktop Agent Guidelines

These are the standing rules for AI agents, contributors and maintainers working
in this repository. Read them before any change. Where they disagree with an
ad-hoc instruction, escalate — don't start rewriting.

## Project Identity

**PixivFlow Desktop is the official desktop runtime environment and release
layer for PixivFlow** — its *distribution*, not its implementation.

- PixivFlow Desktop **manages the runtime**: native desktop experience,
  backend lifecycle, packaged runtime, simplified install & launch for ordinary
  users.
- PixivFlow (upstream repo) **owns the business logic**: scheduling, downloads,
  delivery, media handling, Pixiv auth.

Dividing line:

```
Desktop  = Runtime / Lifecycle / Native Integration
Backend  = Business Logic
```

PixivFlow Desktop is *not*:

- a re-implementation of PixivFlow
- a new Pixiv client
- a generic Tauri Shell framework
- a backend replacement

## Architecture Rules

The fixed three-layer architecture:

```
Desktop Layer
(Tauri + Rust)
      |
      v
Backend Runtime
(PixivFlow)
      |
      v
Web UI
(PixivFlow Frontend)
```

Layer responsibilities:

- **Desktop Layer (this repo)** — window / app lifecycle, process management,
  runtime discovery, configuration, logging, native integration. Thin on
  purpose: no business logic.
- **Backend Runtime (upstream PixivFlow)** — the execution plane: PixivFlow
  business logic, HTTP + Socket.IO API, scheduler, download pipeline.
- **Web UI (upstream pixivflow-webui)** — the user interface, served by the
  backend. The desktop opens it; it never re-implements it.

## Tauri Responsibilities

Rust / Tauri **may** own:

- application lifecycle
- process management (spawn / stop / restart / health)
- runtime discovery
- configuration
- logging
- native integration (window, tray, opener, single-instance, …)

Rust / Tauri **must not** own:

- Pixiv API access
- scraping
- download logic
- scheduler
- any Pixiv business rule

Any Pixiv functionality must live in the PixivFlow backend and be reached over
its API — never reimplemented here.

## Window model

Two windows exist; normally only one is visible.

- **`webui` — the app.** `open_webui_window()` creates a `WebviewWindow` on the
  backend's `http://127.0.0.1:<port>/`, injects the host bridge, and shows it as
  soon as the backend answers healthy (auto-start thread in `lib.rs`).
  **Closing it quits**: the backend is stopped and the process exits.
- **`main` — the fallback panel** (`"visible": false` in `tauri.conf.json`). The
  bootstrap shell is not the front door any more; it appears only when the
  backend never becomes healthy (the `else` branch of the auto-start thread) or
  from the menu bar's *Open manager*. Closing it while a `webui` window exists
  merely hides it.

Never resurrect the pattern "launcher window first, product UI second" — a
manager window opening on every start was reported as a defect.

## Native sign-in (embedded)

Pixiv sign-in stays **inside the app**: never a separate OS window, never the
user's browser.

- The WebUI calls `window.pixivflowHost.openLoginWindow(authUrl, redirectUri)`
  (injected by `commands::HOST_BRIDGE_SCRIPT`), which invokes the Rust command
  `open_login_window`.
- That command embeds the Pixiv authorize page as a **child webview** covering
  the `webui` window (`Window::add_child`, label `login`) — this is why
  `src-tauri/Cargo.toml` enables tauri's `unstable` feature (multi-webview is
  gated behind it). The WebUI page stays loaded underneath, so its pending
  promise resolves normally: the WebUI and the backend need **no** change, and
  the `{ code }` contract is untouched.
- The child webview has no chrome, so cancellation is host-side only: the app
  menu's *取消登录* → `commands::cancel_login()` ends the wait (`None`), as does
  the 300 s timeout.
- The remote Pixiv origin gets **no** capability entry — every `invoke` from it
  is refused by Tauri's ACL. Never grant one to make a probe work.
- The overlay is resized with its parent from the `RunEvent::WindowEvent {
  Resized }` hook and is always closed before the command returns.

## The app menu

The launcher is hidden by default, so the menu bar carries the host controls
(`build_menu()` in `lib.rs`): *Open manager*, *Open logs*, *Collect diagnostics*,
*Cancel sign-in*. A new host-level action belongs in the menu **and** in the
fallback panel — never in the panel alone.

## BackendManager Contract

`BackendManager` is a **pure lifecycle owner**. It is permanently limited to:

- `spawn`
- `adopt` (take over our own runtime that survived a force-quit / crash)
- `stop` (must run on window close **and** on app quit — a macOS quit never
  reaches the window close handler, so `lib.rs` also handles
  `RunEvent::ExitRequested` / `RunEvent::Exit`; `stop()` stays idempotent)
- `restart`
- `health check`
- `status`

Forbidden forever:

- adding business logic
- altering backend behavior
- handling / parsing Pixiv data
- downloading / installing / upgrading the runtime — that belongs to a future
  `RuntimeManager`, never here

Deciding *which command to run* is the job of the discovery/adapter layer
(`src-tauri/src/backend/discovery.rs`), which only resolves a command and injects
it via `set_command_override()`; it does not touch the process lifecycle.

## Frontend Rules

The desktop frontend (`src/frontend`) stays **light** — a control / status shell.
It is the **fallback panel** (`main`), not the product UI: its copy must present
it as backend status, never as the app's main window.

Allowed:

- backend state display
- configuration management
- native controls (buttons → invoke commands)
- Tauri IPC `invoke` commands

Forbidden:

- copying PixivFlow WebUI
- implementing backend business behavior
- creating a second state-management layer for business data

Use the bundled WebUI over `STATIC_PATH` (方案 A) for the actual product UI;
never maintain a parallel frontend.

## Runtime Rules

Fixed discovery priority (highest first):

```
1. bundled runtime     resources/runtime/pixivflow/ (runtime-manifest.json)
2. user configured     backend.command + backend.args
3. PATH runtime        `pixivflow` on PATH
4. mock runtime        (test only)
```

Forbidden:

- implicitly choosing an unknown / unresolved runtime
- silently modifying the user's configuration

The bundled runtime wins so a shipped install is never perturbed by a stray
`pixivflow` on PATH or in config.

Reachability variables **are** injected: `BackendManager::start()` forwards the
system proxy as `HTTPS_PROXY` / `HTTP_PROXY` (read from `scutil --proxy`, never
overriding a value the user or the `LaunchSpec` already set) plus a loopback
`NO_PROXY`. A Finder/Dock launch inherits no environment, and both the login
token exchange and every download happen *inside the backend process* — without
this the backend could never reach Pixiv even though the login webview can.
Otherwise the backend still only receives `STATIC_PATH` (plus `PORT` / `HOST`).

**Storage paths are never injected**: PixivFlow's path auto-fixer
rewrites absolute paths outside its `process.cwd()` back to `./data`, so the
desktop instead spawns the backend **with its CWD set to the per-user data root**
(`app_local_data_dir()/pixivflow`). PixivFlow's own `config/`, `data/` and
`downloads/` defaults then land there.

Adoption rule: `start()` adopts a live listener only when all three hold — the
port is open, `GET /api/health` answers `200`, and the listener's command line
contains `pixivflow`. Anything else stays a hard "port occupied" error, and
adoption is never used to take over a foreign service.

### Runtime manifest contract (F2.3)

`runtime-manifest.json` fields — all except `name`/`command` are optional with
defaults, so old manifests keep parsing:

| field | meaning |
|---|---|
| `name` | runtime name (`pixivflow`) |
| `version` | version string; trusted as-is — never `--version`-probe a bundled runtime (a real entry may boot a server instead of printing) |
| `platform` | `os-arch` tag (e.g. `darwin-arm64`); checked leniently (`aarch64`≡`arm64`, `x86_64`≡`x64`) |
| `command[]` | argv; `command[0]` is the executable, relative paths resolved against the runtime dir |
| `args[]` | fixed argv appended after `command` |
| `health` | health path, default `/api/health` |
| `staticPath` | WebUI static dir (relative to the runtime dir or absolute); when unset/non-existent the desktop falls back to its own `resources/webui/dist` |
| `servesWebui` | whether the backend serves the WebUI over `STATIC_PATH` (方案 A) |

### Resource paths

Installed-app resources must resolve through Tauri
`app.path().resource_dir()` — **never** assume `CARGO_MANIFEST_DIR`, which only
exists on a build machine. Resolution order: installed bundle root →
compile-time manifest dir (dev/tests) → CWD-relative dev path.

Desktop stays a **packaging layer**: never copy PixivFlow backend *source* into
this repo — the fetch script lays built artifacts only. No auto-update yet.
A future release may add `checksums.json` + a download/verify flow (design
only — not implemented).

## Configuration Compatibility

The following are **public interfaces** — treat them as versioned contracts:

- `desktop-config.json`
- the bundled runtime manifest (`runtime-manifest.json`)
- Tauri IPC commands

Any change must consider:

- backward compatibility
- migration path
- release notes

## Logging & diagnostics

The log lives in the **OS log dir** (`~/Library/Logs/dev.redtidev.pixivflowdesktop/`
on macOS), not in the repo; a non-empty `logDir` in `desktop-config.json`
overrides it and a CWD-relative `logs/` is only the last resort. It rotates at
**2 MiB**, keeping `desktop.log.1`…`.3` beside a fresh `desktop.log`, so a user
can always hand over a bounded artifact.

- Timestamps are UTC and explicit: `2026-09-26T18:21:48Z`.
- **Panics are captured**: the hook appends `PANIC <payload> at <file>:<line>`
  plus a forced backtrace to the log and to a sibling `panic-<utc-ts>.log`,
  while the standard stderr message still prints.
- **Run events are traced**: every `RunEvent` is logged compactly, window events
  with their label, but high-frequency focus / scale / move events are filtered
  out so a click or a resize never drowns the log.
- **Never log secrets.** Tokens, codes, passwords and credentials must never
  reach a line; foreign text (panic payloads, frontend messages) goes through
  `logger::redact_secrets` first.
- A `crash-*.ips` copy collected into the log dir is the durable evidence of a
  native crash — macOS retires and later purges the originals from
  `DiagnosticReports/`, so the copy is the only lasting record.
- **`export_diagnostics` is the supported way to collect evidence from a user**:
  one folder with the logs, `last-run.json`, collected crash reports,
  `doctor.json`, a secret-redacted `config.json` and `env.txt`.

### Adding a command

Every app command must appear in **three** places; miss one and the window is
refused at runtime with `Command <name> not allowed by ACL`:

1. the ACL manifest in `src-tauri/build.rs` (`AppManifest::new().commands(&[…])`),
2. the capability that should be allowed to call it
   (`src-tauri/capabilities/*.json`, permission id `allow-<command>`, `_` → `-`),
3. `generate_handler!` in `src-tauri/src/lib.rs`.

Grant the command to the narrowest capability that needs it: the launcher
(`default.json`) gets everything, the remote `webui` window only what the hosted
page legitimately needs.

### Host capabilities, not backend capabilities

Anything that hands information to a *device* — showing a path in Finder or
Explorer, system notifications, clipboard, tray, file associations — belongs to
this layer, never to the PixivFlow runtime. The runtime answers *where* a file
is (`GET /api/files/location`); it must never spawn a desktop program, because
`browser → remote backend → xdg-open` is meaningless on a server and misleads
the user about which machine opens.

The direction of responsibility is therefore one-way: the WebUI resolves and
confines the path through the backend, then asks the host to show it. The host
command validates only what it can see locally (does the path exist) and must
not re-derive, expand or interpret paths.

- `reveal_path(path)` — "Show in Finder" semantics: a file is *selected* in its
  folder (`open -R` on macOS, `explorer /select,` on Windows, `xdg-open` on the
  parent directory elsewhere), a directory is opened. Exposed to the remote
  WebUI as the bridge function `pixivflowHost.revealPath(path)`.
- `notify(title, body, level?)` — raise a system notification. The **WebUI owns
  the wording** (it localises before calling) and the decision to interrupt at
  all (it asks only when the page is in the background); this layer owns whether
  the machine can show one and starts the thing that does. macOS uses
  `osascript -l JavaScript` with the copy passed as **argv** (JXA does not
  escape argv, so quotes/backslashes/Unicode are safe), Linux uses
  `notify-send`; Windows answers `NOTIFY_UNAVAILABLE` because WinRT toast needs
  a registered AppUserModelID. The command returns `{ shown, reason? }` rather
  than an error, because a refused toast is a normal outcome the WebUI must be
  able to report honestly — a notification nobody saw must never be logged as
  delivered. Bridge function `pixivflowHost.notify({ title, body, level })`,
  which maps `NOTIFY_DENIED` → `reason: 'denied'` and everything else →
  `'unavailable'`.
- `open_external(url)` — hand a link to the user's own browser. Bridge function
  `pixivflowHost.openExternal(url)`.
- `open_in_app(url)` — navigate the WebUI window to another page of the *same*
  app. Confined to the window's current origin (scheme, host and port must all
  match), because a page served from the backend must not be able to turn the
  shell into a browser pointed elsewhere. Bridge function
  `pixivflowHost.openUrl(url)`.

The two link commands are **separate promises and must not be merged**: "a real
browser tab the user can see the address of" is right for an external
docs/OAuth/"made with" link and wrong for another PixivFlow page. Both accept
only `http`/`https` and pass the URL as argv — never through a shell — so a query
string containing `&`, `"` or `$` cannot become a second command. The pure
platform matrices live in `src-tauri/src/{reveal,notify,link}.rs` and are
unit-tested on every target (`cfg!`, not `#[cfg]`).

### User-visible strings

Every user-visible string must come from `src/frontend/i18n.js` (both locales;
key parity is enforced by the frontend tests) or `src-tauri/src/i18n.rs` —
never hard-coded at the call site. English must stay byte-identical.

## Development Rules

All modifications:

- prefer small, incremental steps
- preserve the existing architecture
- evolve, don't replace

Forbidden:

- large-scale rewrites
- introducing new frameworks without need
- re-implementing capabilities that already exist upstream

## Testing Requirements

Backend lifecycle changes must verify:

- `cargo test`
- start
- health check
- graceful shutdown (SIGTERM, never `kill -9`)
- adoption: a surviving backend is reused instead of erroring

Packaging changes must be tested from the **installed bundle** (launch the built
`.app`, not only a debug run): the bundled runtime boots, health returns 200, the
WebUI window opens automatically and is the only window on screen, an embedded
Pixiv sign-in can be started and cancelled from the app menu, and closing the
WebUI window stops the backend.

UI / frontend changes must verify:

- `npm run build`
- `cargo build`
- the production bundle (dist + check-dist smoke test + frontend unit tests)

## Current Roadmap

Current stage: **F4.1 — Real runtime + real WebUI in the bundle.** The `.app`
carries the real self-contained PixivFlow runtime (a standalone `node` beside
`dist/` + `node_modules/`), the built WebUI dist, a per-user data root as the
backend CWD, automatic WebUI opening after health, and adoption of a backend
orphaned by a crash. The WebUI window *is* the app (the manager window is a
hidden fallback, see "Window model") and Pixiv sign-in happens in an embedded
view inside it. The F2.3 work formalized the manifest
`{version, platform, command[], args[], health, staticPath, servesWebui}` and
`scripts/fetch-pixivflow-runtime.mjs` lays the real backend into
`src-tauri/resources/runtime/pixivflow/` (git-ignored build product); doctor
reports runtime validation + WebUI status. Discovery resolves resources at
runtime: installed bundle root (`app.path().resource_dir()`) → compile-time
manifest dir → CWD-relative dev path, so the same code works in dev and in the
`.app` / `.AppImage`. The committed default stays the dev stand-in
(`dev-backend.mjs`). Checksums / download / auto-update are design-only.
Remaining F4 per `docs/ROADMAP.md`: runtime release assets (4.2), `RuntimeManager`
(4.3), CI release matrix (4.4), branding (4.5), doctor expansion (4.6), and the
DMG / Windows / Linux installers.

Not currently implemented (do not add without an explicit decision):

- auto-update
- login system
- cloud sync
- AI features
- Pixiv business-logic rework

Full plan: `docs/ROADMAP.md`.

## Commit Rules

A commit should be:

- single-purpose
- revertible (self-contained)
- accurately described

Avoid:

- interleaved "cleanup everything" commits
- architecture overhauls disguised as maintenance
- touching unrelated files

Suggested message style for docs-only work, e.g.:
`docs: add agent guidelines and architecture documentation`

## Working alongside other agents

More than one agent session can be editing this repo at the same time. Treat every
file as shared:

- **Re-read before you commit.** `git status` and `git diff` immediately before
  staging. Your own edit log is not evidence — a file may have changed under you.
- **Never stage, revert, or "clean up" work you did not write.** If a change is
  not yours, leave it alone and say so in the report.
- **One repo, one boundary.** Touch only files inside this checkout. Built
  artifacts (`src-tauri/target/`, a built `.app`) are never committed; the
  git-ignored bundle inputs (`src-tauri/resources/webui/dist`,
  `src-tauri/resources/runtime/pixivflow/{dist,node_modules,node}`) are local
  working state that must be restored to the committed state before committing.
- **Generated/machine-local files stay out of history.** Commit only source and
  docs; never a path pointer, a version stamp, or an assembled bundle.

## Verification gates (do not skip)

Run these before declaring any change done, and report the actual output:

```bash
cargo test                                   # unit + integration
cargo check                                  # no new warnings
npm run test                                 # frontend + dist smoke check
npm run build && git status --porcelain      # build must not dirty the tree
```

For a change that reaches the running app, also build and launch the installed
bundle (`docs/DEVELOPMENT.md` → *Building the app bundle*) and confirm the app
reaches `health OK` with the WebUI window auto-opened. Rust-only unit tests do
not prove that a bridge command is reachable from the remote page: ACL mistakes
only appear at runtime, so a new capability needs at least one real invocation
through `window.pixivflowHost` and its result in the desktop log.

Report in this order: what changed, what was verified (command → result), which
docs moved, the commit hash, and whether it was pushed.

## Cross-repo sync (upstream follows downstream)

This repo is the **downstream** host of two upstreams: `PixivFlow` (backend,
runtime + API contract) and `pixivflow-webui` (the UI this repo serves). A
downstream change routinely reveals an upstream defect, a stale doc, or an
undocumented contract.

Rule: **never leave that upstream work undone or unpushed.** In the same piece
of work:

1. Fix the upstream defect there (e.g. a WebUI i18n/clipping bug is fixed in
   `pixivflow-webui`, not worked around here).
2. Record the contract/staleness upstream in the owning repo's docs — the host
   integration facts belong in PixivFlow's `docs/platform-contract.md`, the
   host/UI-difference facts in `pixivflow-webui`'s docs.
3. Commit and **push** each repo separately (single-purpose commit, its own
   message), and say which upstream commit landed in the report.

Only the *consumption* of that contract lives here; the contract itself is
owned upstream. Use `git -c http.proxy=http://192.168.2.20:7892 push origin HEAD`
where a proxy is needed.

Suggested message style for docs-only work, e.g.:
`docs: add agent guidelines and architecture documentation`