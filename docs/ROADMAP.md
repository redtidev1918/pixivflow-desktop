# Roadmap

PixivFlow Desktop is the **desktop shell** of the PixivFlow ecosystem. Every phase
below is independently releasable; each keeps the desktop a thin shell and never a
rewrite of upstream PixivFlow / webui.

| Phase | Theme | Scope |
|---|---|---|
| **Phase 0 — Foundation** ✅ | Repository skeleton | structure · docs · `desktop-manifest.json` / `desktop-config.json` contracts |
| **Phase 1 — Shell MVP** ✅ | First runnable app | Tauri 2 window · frontend status shell · `BackendManager` (start/stop/restart/health) · config system · single-instance · file logging |
| **Phase 2 — Real backend integration** 🚧 | Lifecycle | 2.1 adapter+discovery+doctor ✅ · 2.2 bundled runtime+manifest+方案 A WebUI (`STATIC_PATH`)+open WebUI ✅ · 2.3 `scripts/fetch-pixivflow-runtime.mjs` acquires the real PixivFlow runtime into the bundle ✅ (committed default stays the dev stand-in) |
| **Phase 3 — UX** ⬜ | Experience | settings page · log viewer · remote mode · tray icon |
| **Phase 4 — Distribution** 🚧 | Packaging | 4.0 runtime resource resolution + icon set + `.app` bundle, smoke-verified on macOS ✅ · DMG · GitHub Actions · Windows .exe · Linux AppImage |
| **Phase 5 — Releasegraph** ⬜ | Fleet automation | `desktop-manifest.json` version lock · automated update PR · auto Desktop release |

## Phase 0 — Foundation ✅

- Repository structure.
- Documentation.
- `desktop-manifest.json` version-lock contract.
- `desktop-config.example.json` — the `{mode, backend, dirs, remote}` contract.

## Phase 1 — Shell MVP ✅ (current)

Delivered and verified in this repo (Tauri 2.x):

- **Window** — main webview, launched via Vite dev server.
- **Frontend shell** — vanilla JS status page (`src/frontend/`), states:
  `starting / running / stopped / error / unknown`, driven by `invoke` + a
  `backend-status` event + a 1s poll.
- **`BackendManager`** (`src-tauri/src/backend/manager.rs`):
  `start()` idempotent (refuses if port taken) · `stop()` graceful SIGTERM +
  wait, never `kill -9` · `restart()` · `health_check()` on `/api/health`.
- **Config system** (`src-tauri/src/config.rs`) — reads `desktop-config.json`
  from the platform app-config dir; auto-creates a default; surfaces parse errors.
- **Commands** (`commands.rs`): `get_status` / `get_config` / `config_path` /
  `start_backend` / `stop_backend` / `restart_backend`.
- **Single instance** — `tauri-plugin-single-instance`.
- **Logs** — `logs/desktop.log` records startup, backend lifecycle and errors.
- **Graceful shutdown** — closing the window stops the backend.
- **Integration test** `tests/backend_test.rs` proves the full
  `start → health → graceful-stop → double-stop no-op` contract headlessly.

> F1 ships a **mock backend** (`src-tauri/resources/mock-backend.mjs`) as the
> health-probe stand-in. The backend command is configurable via
> `backend.command` and is replaced by the real PixivFlow in Phase 2.

### Acceptance checklist (F1)

- [x] `cargo tauri dev` opens a window.
- [x] Backend auto-start simulated (mock) on configurable port → `starting` → `running`.
- [x] Health detected (`/api/health` → 200).
- [x] Closing the window stops the backend gracefully.

## Phase 2.1 — Real backend adapter ✅

Keeps `BackendManager` as a pure lifecycle owner (**spawn / stop / restart /
health**) and adds a **discovery/adapter** layer so it can drive a REAL
PixivFlow backend — resolved, not assumed.

- **`backend/discovery.rs`** — resolves which backend to run, in priority order:
  1. bundled `resources/runtime/pixivflow`
  2. user-configured `backend.command` (+ `backend.args`)
  3. `pixivflow` on `PATH`
  4. fallback: the bundled mock backend (dev/test)
- **Config shape** — `backend.command` is now a plain string and `backend.args`
  an array (matches the desktop-config contract; `mode` is **top-level**, not
  under `backend`):
  `{ "mode": "local", "backend": { "command": "", "args": [], "port": 3000, "autoStart": true } }`.
- **BackendManager** — unchanged lifecycle; only gains a tiny adapter wiring
  point `set_command_override()` so the resolved command can be injected.
- **`backend` doctor command** — reports `backend_found`, `executable_path`,
  `source` (`bundled|config|path|mock`), `version` (from `--version` probe),
  `port`, `running`, `healthy`, `message`. Frontend shows source + version.
- **Version probe** — `discover_with_version()` runs `argv --version` (bounded).
- **Tests** (`backend_test.rs`, 6): adapter parses configured command ·
  fallback resolves · `--version` probe · fake-backend start→health→stop via
  the adapter · doctor components resolve · F1 round-trip still green.

Live-verified: with `backend.command` set, the app logs `backend resolved:
source=config … real=true`, spawns it, and `/api/health` → 200.

## Phase 2.2 — Bundled runtime + WebUI integration (方案 A) ✅

Validates the "ordinary user install & run" story: the shell discovers and
supervises a **bundled** backend and opens the **bundled WebUI** served by that
backend — no second frontend, no Node/Python/Docker requirement for the end user.

- **Runtime layout contract** (`src/resources/…`): `runtime/pixivflow/`
  (`runtime-manifest.json` + `VERSION` + entry), and `webui/dist/` (static
  WebUI). The manifest declares `{name, version, platform, command[], health,
  servesWebui}`.
- **Precedence (F2.2.2)** — discovery now prefers:
  1. **bundled** runtime (manifest-described) — always wins, so a stray
     `pixivflow` on PATH / in config can't perturb a shipped install
  2. user-configured `backend.command` (+ `backend.args`)
  3. `pixivflow` on `PATH`
  4. fallback: bundled mock (dev/test)
- **`LaunchSpec`** — the manager accepts a `{command, env}` override; discovery
  injects `STATIC_PATH=<resources/webui/dist>` for 方案 A hosting. Core
  `BackendManager` lifecycle (spawn / stop / restart / health) is untouched.
- **WebUI serving** — backend serves the static dist over `STATIC_PATH`; the
  desktop loads `http://127.0.0.1:{port}/` in a dedicated `webui` window via the
  new `open_webui` command (reused if already open). The Tauri shell stays the
  *control* surface.
- **Control UI (F2.2.4)** — the desktop shell shows backend status, port, PID,
  health, source, version, mode and offers **打开 PixivFlow** (open WebUI),
  Restart, Stop, Open Logs.
- **Tests** (`backend_test.rs`, 9): bundled manifest parsing · static path ·
  precedence over config/PATH · end-to-end `start → health 200 → GET / serves
  the webui dist → SIGTERM stop` · lifetime round-trips still green.
- **Dev stand-in** — `runtime/pixivflow/dev-backend.mjs` (versioned
  `0.0.0-dev`) proves the full bundled contract without shipping real PixivFlow
  code; the real release binary is wired in F2.3.

Live-verified: default config resolves `source=bundled real=true`, auto-starts
the bundled backend, `/api/health → 200`, and `GET /` serves the bundled WebUI
dist.

## Phase 2.3 — Fetch real PixivFlow runtime ✅

Acquire the real backend and wire it in via the runtime manifest.

- **`scripts/fetch-pixivflow-runtime.mjs`** — `node scripts/fetch-pixivflow-runtime.mjs
  [--source <dir|npm-spec>]`. Builds the real backend (from a local PixivFlow
  checkout, or fetches an npm spec) and lays it into
  `resources/runtime/pixivflow/{dist, node_modules, package.json, VERSION}`,
  rewriting `runtime-manifest.json` to the formal contract `command:["node"]` +
  `args:["./dist/webui/index.js"]`. npm-workspace deps the built dist needs (e.g.
  `@redtidev/pixiv-client`) are materialized so the runtime is self-contained.
- **formal runtime contract** — manifest fields `{name, version, platform,
  command[], args[], health, staticPath, servesWebui}` (only `name`/`command`
  required). `staticPath` may point at a runtime-relative WebUI dir; otherwise the
  desktop falls back to `resources/webui/dist`. Platform tags use the Node
  convention (`darwin-arm64`) and are matched leniently against the host.
- **runtime validation + richer doctor** — `backend_doctor` returns a `runtime`
  report (manifest found/valid, entry exists, version, platform match,
  servesWebui) and a `webui` report (STATIC_PATH on-disk, live `GET /`
  accessible). Checksums/download/auto-update remain **design-only** for the
  future release bundle; the desktop stays a packaging layer (no backend source
  copied in).
- **git-ignored artifact** — the laid-out runtime is a local build product. The
  committed default manifest stays the dev stand-in (`dev-backend.mjs`), so a
  fresh clone runs light until the fetch runs.
- **version from manifest** — `discover_with_version` trusts the manifest version
  (a real release binary may not support `--version`); only version-less sources
  get an `--version` probe.
- **tests robust to both** — `backend_test.rs` exercises the dev stand-in
  explicitly, asserts the bundled-runtime contract agnostically, and validates
  the bundle report, so `cargo test` (10 tests) passes whether or not the real
  runtime is laid out.

Live-verified: fetched real `pixivflow@2.46.0`; the self-contained runtime boots
from `resources/runtime/pixivflow/dist`, `/api/health` → 200, `GET /` serves the
bundled WebUI dist, graceful SIGTERM stop. (Bundling the real *webui dist* remains
an open follow-up — F2.2 step 方案 A keeps the placeholder `webui/dist`.)

## Phase 3 — UX ⬜

- Settings page (`remote` mode, data/download/log dirs).
- Log viewer.
- Tray icon + minimize-to-tray.

## Phase 4 — Distribution 🚧

### 4.0 Packaging closure — macOS first ✅

- **Runtime resource resolution** — discovery resolves bundled runtime / WebUI
  / mock paths against the Tauri `resource_dir()` first, then the cargo
  manifest / CWD paths; the same code works in dev and installed.
- **Bundle config** — `bundle.active=true`, explicit `resources` map with the
  in-bundle layout (`runtime/pixivflow/…`, `webui/dist/…`, mock script) and a
  full icon set (`tauri icon`).
- **Verified** — `tauri build --bundles app` produces
  `PixivFlow Desktop.app`; launched from the bundle the backend auto-starts,
  `/api/health` → 200, `GET /` serves the bundled WebUI, and closing stops it.

### Remaining

- Windows installer (.exe).
- macOS .dmg.
- Linux AppImage.
- GitHub Actions release workflow.
- Real-runtime resource globs (the fetched `dist` / `node_modules`).

## Phase 5 — Releasegraph ⬜

- Add to the releasegraph dependency graph.
- `desktop-manifest.json` version lock → automated update PR when an upstream
  component releases.

## Design constraints (non-negotiable)

1. **Stay a shell** — never copy or re-implement PixivFlow / webui logic.
2. **Don't modify upstream** — configure the backend via env overrides, not patches.
3. **No new microservices.**
4. **Don't fork releasegraph** — build on its primitives.
5. **Never auto-publish untested versions.**