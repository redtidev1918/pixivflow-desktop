# Release & Version Management

## Component version lock

[`desktop-manifest.json`](../desktop-manifest.json) is the **component version
lock** the desktop expects of its upstream dependencies:

```json
{
  "schemaVersion": 1,
  "components": {
    "pixivflow": { "version": "0.0.0" },
    "pixivflow-webui": { "version": "0.0.0" }
  }
}
```

At **F0** the versions are placeholders (`0.0.0`). They get pinned to real
versions only once actual bundling begins (Phase 2).

## Automatic update flow (future — Phase 5)

`releasegraph` will drive the desktop's releases from the upstream lifecycle:

```
PixivFlow release
        |
        |
desktop update PR
        |
        |
Desktop release
```

1. An upstream component (`pixivflow`, `pixivflow-webui`) releases.
2. `releasegraph` detects the stale pin in `desktop-manifest.json`.
3. It opens a **desktop update PR** that bumps the pinned version.
4. After the build gate passes and the PR merges, this repo's release pipeline
   publishes installers + the Tauri updater payload.

Never publish on `latest`/floating tags — pins are immutable versions only.
Never auto-merge an unverified upgrade.