# Roadmap

PixivFlow Desktop is the **desktop shell** of the PixivFlow ecosystem. Every phase
below is independently releasable; each keeps the desktop a thin shell and never a
rewrite of upstream PixivFlow / webui.

| Phase | Theme | Scope |
|---|---|---|
| **Phase 0 — Foundation** ✅ | Repository skeleton | structure · docs · `desktop-manifest.json` / `desktop-config.json` contracts |
| **Phase 1 — Shell MVP** ✅ *(current)* | First runnable app | Tauri 2 window · frontend status shell · `BackendManager` (start/stop/restart/health) · config system · single-instance · file logging |
| **Phase 2 — Real backend integration** ⬜ | Lifecycle | download backend release · bundle webui dist · `STATIC_PATH` · run the real PixivFlow locally |
| **Phase 3 — UX** ⬜ | Experience | settings page · log viewer · remote mode · tray icon |
| **Phase 4 — Distribution** ⬜ | Packaging | GitHub Actions · Windows .exe · macOS .dmg · Linux AppImage |
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

## Phase 2 — Real backend integration ⬜

- Acquire the real backend (downloaded release or local build).
- Bundle the webui frontend dist and serve it (`STATIC_PATH`).
- Run real PixivFlow locally via `backend.command` (no Node/Docker requirement for the end user).

## Phase 3 — UX ⬜

- Settings page (`remote` mode, data/download/log dirs).
- Log viewer.
- Tray icon + minimize-to-tray.

## Phase 4 — Distribution ⬜

- Windows installer (.exe).
- macOS .dmg.
- Linux AppImage.
- GitHub Actions release workflow.

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