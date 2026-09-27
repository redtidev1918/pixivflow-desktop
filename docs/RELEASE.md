# Release & Version Management

The desktop releases through the same fleet engine as every other product repo:
**release-please** decides the version, **releasegraph** builds and publishes it.
This repo owns only three things — the asset contract, the per-runner build, and
the Chinese release notes.

## How a release happens

[`.github/workflows/release.yml`](../.github/workflows/release.yml) delegates to
`redtidev1918/releasegraph/.github/workflows/reusable-release.yml`:

| Trigger | Effect |
| --- | --- |
| pull request | full dry run — `release_please` + `build` on all three runners, nothing published |
| push to `main` | release-please opens/updates the release PR, or publishes if one is already merged |
| hourly schedule | recovery — finishes a release that was interrupted |
| `workflow_dispatch` | manual `version` / `dry_run` / `force` / `repair` / `stage` |

The usual flow is therefore: merge feature PRs → release-please opens
`chore(main): release x.y.z` → add `.github/release-notes/<x.y.z>.md` to that PR
→ merge → the engine tags `vx.y.z`, builds every platform, publishes the GitHub
Release with checksums, and repairs the run if a leg failed.

Recovery is explicit: `repair: true` (or the dispatch input) re-runs an
incomplete release for the version already in the manifest, `stage` labels where
to resume, and `force: true` re-runs a version the engine considers healthy.

## Version files

Three files carry the desktop version and **must always agree** — the
`release-metadata` CI job fails otherwise:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

`release-please` bumps all three through `extra-files` in
[`release-please-config.json`](../release-please-config.json), so a release PR
never needs a manual bump. Two fallbacks exist for when that cannot apply:

- `npm run release:set-version <semver>` writes one version into all three files
  (manual rehearsal).
- `scripts/build-release` runs the same helper when it notices the committed
  version disagrees with `RELEASE_VERSION` — shipping an installer whose bundle
  version contradicts its release tag is worse than rewriting the files during
  the build.

## Asset contract

[`.release-policy.yml`](../.release-policy.yml) is the contract. Exactly one
shipped name per platform, always the `v`-prefixed form:

| Platform | Shipped name |
| --- | --- |
| macOS | `PixivFlow-Desktop-v<version>-macos-<arch>.dmg` |
| Windows | `PixivFlow-Desktop-v<version>-windows-x64-setup.exe` |
| Linux | `PixivFlow-Desktop-v<version>-linux-amd64.deb` (AppImage optional) |

`scripts/build-release` runs once per runner in the policy's `build.matrix` with
`RELEASE_VERSION`, `RELEASE_TAG`, `DRY_RUN` and `RUNNER_OS` in the environment,
and must leave the artifacts under `dist/release/`. It fetches the real backend
runtime before building (`npm run build`'s `postbuild` refuses the committed dev
stand-in) and prepares what the runners do not provide themselves: the Linux
WebKitGTK packages and, if missing, the Rust toolchain.

Known limits, deliberately tracked rather than hidden:

- Bundles are **unsigned** — no code signing or notarization, so macOS needs
  “Open” from the context menu and Windows may warn. This is a known state, not a
  failed install.
- Each runner ships the architecture it built: `macos-latest` produces the
  **arm64** dmg. An Intel dmg is not produced until per-platform runtime
  artifacts land (roadmap F4.2/F4.3) — the bundled runtime payload contains the
  runner's own `node` binary, so it cannot be cross-built.
- The Windows and Linux legs have never been exercised before this pipeline
  existed. Their first proof is a pull request, whose release run is a dry run.

## Release notes

`.github/release-notes/<version>.md`, in Chinese, keeping the `## 本次更新`
heading (see that directory's README). `release.notes.language: "zh"` in the
policy makes the engine use them instead of the English commit lines, and the
`release-metadata` CI job refuses a release PR whose notes are missing or not
Chinese.

## Docs site after a release

The policy's `release.postRelease` runs two workflows once the release is
published, both required:

1. `update-download-page.yml` (`inputs: {tag: "{{tag}}", deploy-docs: "false"}`)
   — regenerates `docs/download.md` and `docs/en/download.md` from the new
   release via `.github/scripts/update_download_page.py`, using
   `.github/scripts/download-page.json` and the `docs/download-preview.md`
   snippet, then commits the refresh.
2. `docs.yml` — redeploys https://redtidev1918.github.io/pixivflow-desktop/
   (Pages source: GitHub Actions).

So the published docs always describe the newest release; a failed or missing
download page is a release failure, not a cosmetic issue. Neither the download
pages nor the managed files (`docs/index.html`, `docs/assets/vendor/`, the two
workflows and the update script) are hand-edited — regenerate them with
`python3 docsite.py update` / `python3 docsite.py check`.

## Component version lock

[`desktop-manifest.json`](../desktop-manifest.json) records which upstream
versions this build bundles:

```json
{
  "schemaVersion": 1,
  "components": {
    "pixivflow": { "version": "3.1.0" },
    "pixivflow-webui": { "version": "2.0.0" }
  }
}
```

It is an **expected-state declaration, not build input**: the build takes the
runtime from the sibling checkout or npm and the WebUI from its dist. It exists so
the dependency chain stays inspectable and so a future upgrade bot has a place to
write the version it intends to move to.

## Automatic upstream upgrades (future — Phase 5)

```
PixivFlow release
        |
        |
desktop update PR   (bumps desktop-manifest.json)
        |
        |
Desktop release
```

`releasegraph` is meant to detect the stale pin in `desktop-manifest.json`, open
an update PR that bumps it, and let *this* repo's ordinary release pipeline do the
rest once the PR merges. That bot does not exist yet: the design and its
releasegraph-side gaps are written up in
[`docs/analysis/RELEASE-ECOSYSTEM-ANALYSIS.md`](analysis/RELEASE-ECOSYSTEM-ANALYSIS.md)
and [`docs/analysis/releasegraph_PHASE1-audit.md`](analysis/releasegraph_PHASE1-audit.md).

Rules that stay true whatever automates this:

- Never publish on `latest`/floating refs — pins are immutable versions only.
- Never auto-merge an unverified upgrade: the build gate runs before the merge,
  never after it.
- The desktop is a *product* release, not a deploy: nothing here pushes to
  production. The PixivFlow scheduler pin lives in the deploy repo's own
  operations contract.
