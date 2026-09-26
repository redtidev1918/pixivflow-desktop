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

1. bundled `resources/runtime/pixivflow` (shipped inside the app)
2. user-configured `backend.command` + `backend.args`
3. `pixivflow` found on `PATH`
4. fallback: bundled mock backend (dev/testing only)

Resolution result (`BackendDescriptor{source, executable_path, command, version}`)
is injected into the manager via `set_command_override()` before `start()`; the
manager's own fallback (config `command` → mock) is unchanged. The `backend_doctor`
Tauri command surfaces the same resolution plus live runtime status
(`running` / `healthy`), with `version` probed by running `<command> --version`
under a timeout.

Backend config shape (flat, per the desktop-config contract):

```json
{ "backend": { "mode": "local", "command": "", "args": [], "port": 3000, "autoStart": true } }
```
