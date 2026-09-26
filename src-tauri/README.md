# src-tauri

The **Tauri shell** (Rust) of PixivFlow Desktop.

At **F0 (Foundation)** this directory is a **placeholder only** — it contains
`src/README.md`, `README.md` and nothing runnable. Per the F0 scope, **no
`cargo init`, no Rust source, no `tauri.conf.json`** is created at this stage.

From **Phase 1 (`Shell MVP`) and Phase 2 (`Backend integration`)** the Tauri
project is scaffolded here:

- Tauri main window + `BackendManager` (start / stop / restart / health check).
- Bundling of `PixivFlow` + `pixivflow-webui` into `resources/`.
- Tauri updater wiring.

## F0 note

No business logic, no lifecycle manager and no Rust yet. This is intentionally
empty until Phase 1.