# Roadmap

PixivFlow Desktop is the desktop shell of the PixivFlow ecosystem
(**PixivFlow backend** + **pixivflow-webui**, orchestrated by **releasegraph**).
The goal is a tightly-scoped, maintainable launcher — never a rewrite of its
components. Each phase is independently releasable and gated by CI + releasegraph.

## Legend

- ✅ shipped (foundation) · ⬜ planned · 🧪 experimental

## F0 — Foundation ✅

- Repository skeleton per the approved structure.
- MIT license, README, architecture/dev docs, roadmap, contributing & security
  guidelines, issue/PR templates.
- `desktop-manifest.json` component lock (`schemaVersion` 1) for the
  releasegraph-driven upgrade flow.
- `desktop-config.json` example config reference.

## F1 — Shell MVP ⬜

- Tauri main window + `BackendManager` skeleton (`start` / `stop` / `restart` /
  `healthCheck` commands and events).
- Minimal splash / error shell UI (`src/frontend`).
- Loopback same-origin load of the bundled WebUI once the backend is healthy.

## F2 — Local integration ⬜

- Bundle `PixivFlow` + `pixivflow-webui` per `desktop-manifest.json`.
- Launch backend with `STATIC_PATH` → bundled WebUI; same-origin `/api` +
  `/socket.io`, zero CORS, zero upstream changes.
- Robustness: port reuse/occupancy detection, duplicate-instance adoption,
  crash detection and recovery (graceful SIGTERM first — never `kill -9` first).

## F3 — Configuration & remote mode ⬜

- `desktop-config.json` read/write surfaced in a small settings window.
- `remote` mode: load an existing PixivFlow WebUI URL without a local backend
  (keeps Docker/host-deployed users intact).

## F4 — Distribution ⬜

- Proper icon set; code signing & macOS notarization (self-signed fallback with
  a clear warning where not configured).
- Multi-platform installers: Windows `.exe`, macOS `.dmg`, Linux `.AppImage`.

## F5 — Automatic updates ⬜

- Tauri updater reading **GitHub Releases** (no self-hosted updater server).
- Signed update payloads alongside release assets.
- Update UX: notify → download → apply → relaunch.

## F6 — releasegraph integration ⬜

- Add `pixivflow-desktop` to `releasegraph/fleet.yaml`.
- Add `.release-policy.yml` + `release.yml` (a pinned call to the shared
  `reusable-release.yml`, mirroring sibling repos — no forking of stable workflows).
- Upstream component bump → merge-queue **upgrade PR** (`build.test` gate) →
  breaking/migration detection routes to human → on merge, the desktop's own
  standard release pipeline produces installers + updater payload.
- Tauri updater validates against the released asset signatures.

## F7 — v1.0 polish ⬜

- `CHANGELOG.md` discipline (semantic versions per
  [Keep a Changelog](https://keepachangelog.com/)).
- Signed releases across all three platforms, end-to-end auto-update verified.
- Documentation polish and maintainability review.

---

## Design constraints (non-negotiable)

1. **Stay a shell** — never copy or re-implement PixivFlow / webui logic.
2. **Don't modify upstream** — the desktop uses env vars (`PORT`, `HOST`,
   `STATIC_PATH`, `PIXIV_DATABASE_PATH`, `PIXIV_DOWNLOAD_DIR`, …), not patches.
3. **No new microservices** — the desktop, its subprocesses, and releasegraph
   are the whole topology.
4. **Don't fork releasegraph** — build on its primitives (fleet, policy,
   reusable workflows, postRelease).
5. **Never auto-publish untested versions** — every release passes the build
   gate before it reaches installers.