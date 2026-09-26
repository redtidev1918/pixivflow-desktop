# Roadmap

PixivFlow Desktop is the **desktop shell** of the PixivFlow ecosystem. Every phase
below is independently releasable; each keeps the desktop a thin shell and never a
rewrite of upstream PixivFlow / webui.

| Phase | Theme | Scope |
|---|---|---|
| **Phase 0 — Foundation** ✅ *(current)* | Repository skeleton | repository structure · documentation · `desktop-manifest.json` config/version contracts |
| **Phase 1 — Shell MVP** ⬜ | First runnable app | Tauri window · frontend shell |
| **Phase 2 — Backend integration** ⬜ | Lifecycle | `BackendManager` · start · stop · health check |
| **Phase 3 — Packaging** ⬜ | Distribution | Windows installer · macOS dmg · Linux AppImage |
| **Phase 4 — Update system** ⬜ | Updates | Tauri updater |
| **Phase 5 — Releasegraph integration** ⬜ | Fleet automation | dependency graph · automated update PR |

## Phase 0 — Foundation ✅ (current)

- Repository structure.
- Documentation.
- `desktop-manifest.json` version-lock contract.

Nothing is runnable yet — no Tauri project, no Rust, no business logic.

## Phase 1 — Shell MVP ⬜

- Tauri main window.
- Frontend shell.

## Phase 2 — Backend integration ⬜

- `BackendManager`.
- `start` / `stop` / health check.

## Phase 3 — Packaging ⬜

- Windows installer (.exe).
- macOS dmg.
- Linux AppImage.

## Phase 4 — Update system ⬜

- Tauri updater (GitHub Releases).

## Phase 5 — Releasegraph integration ⬜

- Add to the releasegraph dependency graph.
- Automated update PR when an upstream component releases.

## Design constraints (non-negotiable)

1. **Stay a shell** — never copy or re-implement PixivFlow / webui logic.
2. **Don't modify upstream** — configure the backend via env overrides, not patches.
3. **No new microservices.**
4. **Don't fork releasegraph** — build on its primitives.
5. **Never auto-publish untested versions.**