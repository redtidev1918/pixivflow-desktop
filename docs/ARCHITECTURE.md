# Architecture

PixivFlow Desktop is a **shell** around the existing PixivFlow ecosystem. It
introduces no business logic of its own; it layers a desktop experience on top of
the stable upstream components.

## Layer responsibilities

### Desktop Layer (this repo / Tauri)

- **Window management** — the Tauri window hosting the WebUI.
- **Backend lifecycle** — start / stop / restart / health-check of the bundled
  or remote PixivFlow backend process.
- **Configuration** — read/write the desktop config (`local` / `remote` mode).
- **Update channel** — the Tauri updater reading GitHub Releases.

The Desktop Layer is deliberately thin. It does **not** contain business logic:
no scheduling, no downloads, no delivery, no media handling, no Pixiv auth.

### Backend Layer (upstream PixivFlow)

- **PixivFlow business logic** — the execution plane.
- **API** — the WebUI-facing HTTP + Socket.IO contract.
- **Scheduler** — the slot ledger / scheduling engine.
- **Download** — the media parsing and download pipeline.

### Web Layer (upstream pixivflow-webui)

- **UI** — the user interface.
- **User interaction** — everything the user sees and clicks.

The desktop renders this UI in its Tauri window via the backend's same-origin
serving; it never re-implements the interface.

## Boundary (non-negotiable)

- Desktop ⇢ launches / supervises / updates the backend. Never replaces it.
- Desktop ⇢ renders the WebUI. Never re-implements it.
- Desktop ⇢ reads historical analysis/docs. Never imports upstream source.
- Business logic belongs to upstream PixivFlow — **Desktop 不包含业务逻辑**.

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
   `runtime-manifest.json` (`{name, version, platform, command[], health,
   servesWebui}`); a legacy bare `pixivflow`/`pixivflow.exe` executable is
   accepted as a fallback.
2. **user-configured** `backend.command` + `backend.args` (user override).
3. `pixivflow` found on `PATH`.
4. fallback: bundled **mock** backend (dev/testing only).

Resolution result (`BackendDescriptor{source, executable_path, command,
version, static_path, serves_webui}`) is injected into the manager via
`set_command_override(Some(LaunchSpec{command, env}))` before `start()`; the
`LaunchSpec` carries the extra env (`STATIC_PATH=<resources/webui/dist>`) needed
for 方案 A WebUI serving. The manager's own fallback (config `command` → mock) is
unchanged. The `backend_doctor` Tauri command surfaces the same resolution plus
live runtime status (`running` / `healthy`), with `version` probed by running
`<command> --version` under a timeout.

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
