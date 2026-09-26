# Contributing to PixivFlow Desktop

Thanks for your interest in contributing. This project is a **desktop shell** —
a thin, cross-platform launcher around the existing PixivFlow backend and
`pixivflow-webui`. Keeping it thin is a design goal, and the whole ecosystem is
orchestrated by [releasegraph](https://github.com/redtidev1918/releasegraph).
Read this before opening a PR.

## First: understand the boundary

| Do                                | Don't                            |
| --------------------------------- | -------------------------------- |
| Manage backend lifecycle, config  | Re-implement PixivFlow logic     |
| Render the bundled WebUI          | Modify `pixivflow-webui` source  |
| Add shell UI (splash/settings)    | Copy upstream code into this repo |
| Wire release/updates via releasegraph | Fork or rewrite stable workflows |

If your change would duplicate business logic or touch an upstream repo through
this one, step back and discuss it in the issue first.

## Setup

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for the full environment setup.

```bash
git clone https://github.com/redtidev1918/pixivflow-desktop.git
cd pixivflow-desktop
npm install          # installs @tauri-apps/cli
# build the bundled upstream assets (requires the sibling repos / $PIXIVFLOW_DIR):
npm run bundle
npm run dev          # or: npm run build
```

> You need a Rust toolchain (`rustup`, `rust-toolchain.toml` honoured) and the
> platform system dependencies for Tauri 2. See the Tauri prerequisites guide.

## Development workflow

- `main` is the single stable branch. Feature branches are welcome; keep PRs small.
- Every PR should reference an issue and describe what it changes and why.
- Add or update docs alongside behavior changes.
- `desktop-manifest.json` bumps and `CHANGELOG.md` are handled through release
  PRs, **not** ad-hoc commits — component pins are release-critical.

## Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/).

- `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `test:`, `build:`, `ci:`

Examples:

```
feat(backend): add restart timeout config
fix: stop backend on window close before exit
docs: document remote mode
```

## Code style

- Rust: run `cargo fmt` and keep `cargo clippy -- -D warnings` clean.
- TS/JS shell UI: keep it minimal and dependency-free where possible.
- Prefer the smallest change that keeps the shell thin.

## Testing

- Run `cargo check` / `cargo test` for the Rust side where feasible.
- Shell UI is intentionally minimal; manual launch checks in `tauri dev` are the
  baseline for F0–F2.
- Release/CI build verification runs on the GitHub Actions matrix.

## Release process

Releases are driven by releasegraph:

1. An upstream (`PixivFlow` / `pixivflow-webui`) or the desktop itself releases.
2. `desktop-manifest.json` is bumped and a merge-queue upgrade PR is opened.
3. After the gate passes and the PR merges, the desktop's own release pipeline
   builds the `.exe` / `.dmg` / `.AppImage` and publishes to GitHub Releases.

Feel free to open an issue if the flow looks off or is under-documented.

## Code of conduct

Be respectful and constructive. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).