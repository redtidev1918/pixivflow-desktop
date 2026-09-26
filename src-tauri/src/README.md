# src-tauri/src

Rust source directory of the Tauri shell — **reserved**, empty at F0.

Intended layout from **Phase 1**:

- `main.rs` — app entry.
- `lib.rs` — Tauri builder / commands.
- `backend.rs` — `BackendManager` (process lifecycle).
- `config.rs` — desktop config load/save.
- `health.rs` — backend health checks.

Per F0 scope, **no Rust is written at this stage**.