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

## BackendManager Contract

`BackendManager` is a **pure lifecycle owner**. It is permanently limited to:

- `spawn`
- `stop`
- `restart`
- `health check`
- `status`

Forbidden forever:

- adding business logic
- altering backend behavior
- handling / parsing Pixiv data

Deciding *which command to run* is the job of the discovery/adapter layer
(`src-tauri/src/backend/discovery.rs`), which only resolves a command and injects
it via `set_command_override()`; it does not touch the process lifecycle.

## Frontend Rules

The desktop frontend (`src/frontend`) stays **light** — a control / status shell.

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

UI / frontend changes must verify:

- `npm run build`
- `cargo build`
- the production bundle (dist + check-dist smoke test + frontend unit tests)

## Current Roadmap

Current stage: **F2.3 — Real PixivFlow runtime contract** — done. The manifest
formalizes `{version, platform, command[], args[], health, staticPath,
servesWebui}`; `scripts/fetch-pixivflow-runtime.mjs` lays the real backend into
`src-tauri/resources/runtime/pixivflow/` (git-ignored build product); doctor
reports runtime validation + WebUI status. The committed default stays the dev
stand-in (`dev-backend.mjs`), so a fresh clone runs light until the fetch runs.
Checksums / download / auto-update are design-only. Next: Phase 3 UX / Phase 4 distribution.

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