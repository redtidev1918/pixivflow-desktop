# Relationship to the ecosystem

PixivFlow Desktop is the **consumer-most** desktop shell of the PixivFlow
ecosystem. It depends on the two stable upstream components and sits beneath the
`releasegraph` orchestration layer.

```
PixivFlow
        |
pixivflow-webui
        |
pixivflow-desktop

releasegraph 管理版本关系
```

| Component | Role for the desktop |
|---|---|
| **PixivFlow** (backend) | Provides all business logic, API, scheduler and download. The desktop launches/supervises it. |
| **pixivflow-webui** | Provides the entire UI. The desktop renders it in a window. |
| **releasegraph** | Orchestrates versions and release pipelines; reads `desktop-manifest.json` to drive desktop auto-upgrades. |

## Rules of engagement

1. **The desktop never owns business logic.** Scheduling, downloads, delivery,
   media and Pixiv auth all live upstream.
2. **The desktop never forks or rewrites upstream.** It configures via env
   overrides (`PORT`, `HOST`, `STATIC_PATH`, `PIXIV_DATABASE_PATH`,
   `PIXIV_DOWNLOAD_DIR`, …) — never patches.
3. **Compatibility with PixivFlow is always preserved.** Changes to the desktop
   must not break existing PixivFlow / webui / Docker / deploy workflows.
4. **Version relationship is managed by releasegraph**, through the
   `desktop-manifest.json` lock (see [RELEASE.md](RELEASE.md)).

## Two config modes

- `mode: "local"` — the desktop starts a local PixivFlow backend, waits for it to
  be healthy, then loads its WebUI.
- `mode: "remote"` — the desktop points at an *existing* PixivFlow server URL
  (keeps Docker / host-deployed users on the desktop with zero migration).

See [`desktop-config.example.json`](../desktop-config.example.json).