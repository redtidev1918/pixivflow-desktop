# Release / CI / Versioning Audit — redtidev1918/pixivflow-webui

Read-only audit. Repo cloned at `/Users/jubaofeng/Documents/Code/pixivflow-webui`, branch `master` @ `d4452bf91ca6811f34f77dd556f5acba0e8fa820` (2026-09-21).

---

## 1. GitHub Actions workflows

`.github/workflows/` contains **5 workflows** (plus `.github/dependabot.yml` and `.github/scripts/{update_download_page.py, download-page.json}`).

### `.github/workflows/release.yml` — **the main release orchestrator**
- **Triggers (verbatim):**
  ```yaml
  on:
    pull_request:
    push:
      branches: [master]
    schedule:
      - cron: "19 * * * *"
    workflow_dispatch:
      inputs:
        version: {description: "Existing version; blank reads the manifest", ...}
        dry_run: ...
        force: ...
        repair: ...
        stage: ...
  ```
- **Job:** one job `release` that calls the **releasegraph reusable workflow** (repo `redtidev1918/releasegraph`), pinned to immutable SHA:
  ```yaml
  jobs:
    release:
      uses: redtidev1918/releasegraph/.github/workflows/reusable-release.yml@164413cb765a9cd404e73b2203cf7165717ca5a8 # ReleaseGraph 164413c
      with:
        version: ${{ inputs.version || '' }}
        dry_run: ${{ github.event_name == 'pull_request' || inputs.dry_run || false }}
        force: ${{ inputs.force || false }}
        repair: ${{ github.event_name == 'pull_request' || inputs.repair || false }}
        stage: ${{ inputs.stage || 'all' }}
      secrets: inherit
  ```
  Net effect: on every push to `master`, every PR, every hour (`cron "19 * * * *"`), and on demand, it runs ReleaseGraph's provider-reconciliation release pipeline (release-please-based version bump → builds dist → publishes GHCR image → post-release actions). `dry_run` is auto-`true` on PR runs and `repair` too.
- **Permissions:** `contents`, `pull-requests`, `issues`, `packages`, `id-token`, `actions` all `write`.

### `.github/workflows/branch-contract.yml`
- Trigger: `on: [pull_request]`.
- One job calling another ReleaseGraph reusable:
  ```yaml
  uses: redtidev1918/releasegraph/.github/workflows/reusable-branch-contract.yml@0b0c28990abac59aa5ae1d2c50bf95ce06d26a8d # ReleaseGraph v1.4.11
  ```
  Net effect: gate that hard-fails violating cutover/release/hotfix/ops PRs against base (production-operation branch contract).

### `.github/workflows/update-download-page.yml`
- Triggers: `workflow_call` (with `tag`/`deploy-docs` inputs — used by ReleaseGraph post-release), `workflow_dispatch`, and `release: types: [published]`.
- Regenerates `docs/download.md` via `.github/scripts/update_download_page.py`, commits with GITHUB_TOKEN, then explicitly dispatches the docs workflow (`docsWorkflow` in `.github/scripts/download-page.json` = **`static.yml`**) because token-pushed commits don't retrigger push workflows.
- Header note: `release: published` only fires for PAT/App-created releases; GITHUB_TOKEN releases must go through entry 1 (workflow_call).

### `.github/workflows/static.yml`
- Deploy **docs/** only to GitHub Pages (not the app).
- Triggers: `workflow_dispatch`, and `push` to `master` with `paths: ['docs/**']`.
- Uses official `configure-pages`/`upload-pages-artifact`/`deploy-pages` v5/v6/v7 actions.

### `.github/workflows/notify-profile.yml`
- On `push` to `master`: dispatches `event_type=update-readme` to the user's profile repo `redtidev1918/redtidev1918` via `PROFILE_REPO_TOKEN`.

**No** dedicated `PR title lint`, no `publish npm`, no `npm publish` step. The only artifact publishers are ReleaseGraph (GHCR) and GitHub Pages (docs only).

---

## 2. Release flow

**Release-please is configured but pushed through ReleaseGraph's reusable workflow**, not standalone release-please.

- `.release-please-manifest.json`: `{ ".": "1.1.0" }` (current version lock file).
- `release-please-config.json` (verbatim): `release-type: "node"`, `package-name: "pixivflow-webui"`, `include-v-in-tag: true`, `include-component-in-tag: false`, **`skip-github-release: true`**.
- The actual release pipeline entry is the `release.yml` job calling:
  ```yaml
  redtidev1918/releasegraph/.github/workflows/reusable-release.yml@164413cb...
  ```
  (quoted fully in §1). So a version is released by pushing to `master` (or hourly cron, or `workflow_dispatch` with `version`/`force`/`repair`/`stage` inputs), and ReleaseGraph reconciles the release PR via release-please and publishes the container.
- `.release-policy.yml`: `versioning.mode = release-please`; `kind = container`; registries: **GitHub required** + **GHCR required** (`ghcr.io/redtidev1918/pixivflow-webui`, `Dockerfile`, platforms `linux/amd64,linux/arm64`, verify via `docker buildx imagetools inspect`). Retention: 1 stable, 1 prerelease, 2 failed_draft, `pruneStable: true`. `sbom: true`, `checksums: false`. Build cmds: test `npm ci --legacy-peer-deps && npx jest ... `, build `npm ci --legacy-peer-deps && npm run build`. Production-ops branches: `chore/cutover-*`, `ops/*`, `release/*`, `hotfix/*` with `requireLatestBase: true`.

You would not call release-please directly; you either invoke `redtidev1918/releasegraph`'s reusable release (same as this repo) or drive release-please in the release-please config file of a downstream pipeline that consumes this package's version.

---

## 3. Tag strategy

- `git tag --sort=-creatordate | head -40` → only **two tags**: `v1.1.0`, `v1.0.1`. Format is `v<semver>` (include-v-in-tag true).
- **Latest tag `v1.1.0` resolves to commit `43ad36ef00b82e6c080232b8d4191505df83552a`, while branch HEAD is `d4452bf9...` — the tag is 16 commits behind HEAD.**
- Tags are **not kept in sync with live releases**: `v1.1.0` was tagged 2026-09-18 (per CHANGELOG) and 16 commits of `master` postdate it untagged. The manifest/package version (`1.1.0`) matches the newest tag, so tag↔manifest are consistent, but release cadence lags branch activity (release-please only bumps on merge/release PR; intervening commits accumulate).

---

## 4. Package version management & publishing

- `/Users/jubaofeng/Documents/Code/pixivflow-webui/package.json` → **`"version": "1.1.0"`**, `name: pixivflow-webui`, MIT. Scripts: `build` = `tsc && vite build`; `dev`/`preview`/`lint`/`test`/`test:e2e:*`/`format*`. **No `private:true`, no `publishConfig`, no `files` field, no `VERSION` file** → not set up for npm publish.
- **Main deliverable is the static `dist/` (React + Vite).** `vite.config.ts`: `outDir: 'dist'`, `sourcemap: true`, **`base: './'`** (relative base for sub-path serving). Build-time injection: `define: { __VITE_API_BASE_URL__: JSON.stringify(env.VITE_API_BASE_URL ?? '') }`.
- **Two expected consumption paths** (per README §构建 and `docs/BUILD_OPTIONS.md`):
  1. **Standalone static hosting / Nginx / CDN** — host `dist/`, reverse-proxy `/api` and `/socket.io` to the backend. When `VITE_API_BASE_URL` is empty the app hits relative `/api` (same-origin) via the proxy.
  2. **Embedded in the PixivFlow main-repo Docker image** — the main repo `Dockerfile` copies this source (local `webui-frontend/` dir, else `git clone --depth 1` of this repo), runs `npm ci && npm run build`, and Express serves the resulting `dist/` at runtime (same-origin, no reverse proxy needed). `STATIC_PATH` is *not* referenced in this repo; the main repo decides the serving path.
- **This repo ALSO publishes its own GHCR image** (`ghcr.io/redtidev1918/pixivflow-webui`, `v*` tags, amd64+arm64) per README:120 and `.release-policy.yml`. The Dockerfile builds `dist/` in a `node:22-alpine` builder and serves it from nginx (`nginx:1.27-alpine`), reverse-proxying `/api` and `/socket.io` to `$UPSTREAM_API` (default `http://host.docker.internal:3000`), with `HEALTHCHECK` and Authorization-header passthrough.
- **Doc contradiction worth flagging:** `docs/DEVELOPMENT_GUIDE.md:88` claims "本仓库作为主仓库的可选组件存在,不单独发布镜像" (does NOT publish its own image), while `README.md:120` and `.release-policy.yml` say it **does** publish `ghcr.io/redtidev1918/pixivflow-webui`. One is stale.
- `.dockerignore` excludes `.git`, `.github`, `dist`, `node_modules`, `docs`, `e2e`, test/coverage artifacts — so the Docker builder rebuilds from source (GitHub ref context), fine for GHCR.

So the build output's required location is `dist/` (from `npm run build`), served either by external static hosting or by the main repo's Express; and an independent GHCR image is the automated artifact. Nothing publishes `dist/` as a downloadable CI artifact.

---

## 5. Manifest / downstream linkage

- **`.release-please-manifest.json`** — the version lock: `{ ".": "1.1.0" }`. This is the file a downstream auto-release system should read for "current released version."
- **`.release-policy.yml`** — exists (quoted in §2); container registry policy, `mode: release-please`.
- **No `VERSION` file.** CHANGELOG.md only has up to `1.1.0`.
- **Backend version lock / coupling:** there is **no declared dependency or version-lock on the PixivFlow backend version** anywhere in this repo. The frontend just targets `<backend>/api` and `/socket.io`. Coupling is by **API contract**, not version manifest (see §6).
- **Runtime env coupling:**
  - `VITE_API_BASE_URL` → build-time, injected by Vite as global `__VITE_API_BASE_URL__`; `client.ts:47-62` prepends `<base>/api` (priority: injected base → env/global mock → relative `/api`).
  - `VITE_DEV_API_PORT` → dev-only proxy/socket port (default 3000), not bundled.
  - Socket.IO uses **relative same-origin** connect in production (`socket.ts:45-46` NODE_ENV production/test → `''`); dev connects to `http://localhost:<devPort>`.
  - `__VITE_ENV__` is only a **test/JS global mock** (`src/test/setupTests.ts:15`, `client.ts:7`, `socket.ts:32`), not a runtime feature — no externally-injectable `__VITE_ENV__`.

---

## 6. Breaking-change / auth / config coupling

Frontend assumptions that could break if the backend changes:

- **Auth endpoints (hardcoded paths):** `src/services/api/auth.ts`:
  - `POST /auth/login` (username/password or proxy config; timeout 30s headless / 10min interactive)
  - `POST /auth/login-with-token`
  - `POST /auth/refresh`
  - `POST /auth/logout`
  - `GET /auth/status` → `AuthStatus { isAuthenticated, user? }`
  - **No Bearer-token injection anywhere** — `interceptors.ts:10-18` is a commented-out stub (`config.headers.Authorization = \`Bearer ${token}\``) and never active. The **frontend never sets an Authorization header**; auth is handled by the running Express host (Basic Auth, server-side).
- **Auth localStorage key `auth-storage`** (`src/stores/authStore.ts:91`, persisted via zustand `persist`, `partialize` keeps `isAuthenticated/userId/username/token/tokenExpiry`). A backend auth-mode change (e.g. switching from Basic-auth-to-Express to per-request tokens) would leave this token in localStorage inconsistent with the server.
- **Socket.IO paths:** single shared connection to `/socket.io` (helpers `acquireSocket`/`releaseSocket`, transports `websocket, polling`). Channels: **`logs`** (events `initial|new`), **`download`** (task snapshot shaped like `GET /api/download/status`). If the backend renamed the socket namespace/channels or the download-snapshot payload shape, this breaks.
- **REST surface:** `docs/DEVELOPMENT_GUIDE.md:94` pins "**REST 共 52 个端点** + `/api/health` (+`/health` alias)", contract documented in the main repo `docs/API.md`. Endpoints referenced client-side include `/api/files/preview?path=...` (`HistoryTable.tsx:40`, `FilePreview.tsx:83`) and `/api/download/status`.
- **Basic-Auth passthrough:** `nginx.conf` forwards `Authorization` verbatim; README:148-149 says if the backend enables `WEBUI_USERNAME`/`WEBUI_PASSWORD`, the browser's auth header flows through the proxy — so the frontend relies on standard HTTP-basic being present, not on any JS auth. Removing basic auth on the backend won't break JS, but the `login`/token flows in `auth.ts` suggest the backend also exposes its own Pixiv-token API.
- **Same-origin requirement:** In the standalone/static deployment the frontend is effectively unusable without a `/api`+`/socket.io` reverse proxy unless `VITE_API_BASE_URL` was baked at build time (docs: "静态部署不能靠环境变量改写 API 地址"). A backend that stops proxying/serving one of these two paths breaks the SPA.

---

## 7. Known issues relevant to a downstream auto-release system

1. **Tag↔HEAD drift**: 16 commits on `master` are untagged behind `v1.1.0`; release-please bumps only at release time, so "latest branch commit" ≠ "latest release/tag". A downstream tracker keyed to `HEAD` will see material no release covers; key to the manifest (`1.1.0`) + latest `v*` tag instead.
2. **Single timestamped hourly trigger** (`cron 19 * * * *`) + PR/`push` to `master` → the releaseables run very frequently; rely on `dry_run` semantics (`github.event_name == 'pull_request'` forces dry_run and repair) so PR runs don't publish.
3. **Two artifact surfaces**: publishes BOTH an independent GHCR image `ghcr.io/redtidev1918/pixivflow-webui` AND becomes embeddable as a component in the main PixivFlow Docker image (which shallow-clones this repo). Auto-release must decide which is canonical; they can drift (image built from repo vs main-repo build arg).
4. **Contradictory docs** on whether this repo standalone-publishes an image (`DEVELOPMENT_GUIDE.md:88` "不单独发布镜像" vs `README.md:120` + `.release-policy.yml` "GHCR required"). An ingest layer parsing docs for registry intent will get conflicting signals — use `.release-policy.yml` as the source of truth.
5. **`release: published` flows only for PAT/App-created releases** (GITHUB_TOKEN-created releases don't fire it) — a downstream system creating releases with GITHUB_TOKEN must call `update-download-page.yml`/releasegraph post-release directly, not rely on the event.
6. **No npm publishing config** (no `private`, no `publishConfig`, no `files`) — this package is consumed as source/`dist`/GHCR image only; don't wire it to an npm registry.
7. **Frontend/backend are coupled by implicit API contract, not a version** — no backend-version check exists in the frontend. The main-repo build (`git clone --depth 1` of this repo) can silently mix incompatible major versions of frontend and backend. A downstream auto-release should publish/consume the GHCR image or mint explicit version-lock manifests rather than assume contract stability.
8. **`NOTIFY` / docs-side effects** on every `master` push (`notify-profile.yml` dispatch; Pages deploy when `docs/**` changes) — extra out-of-band writes the release pipeline must not fight with.

---

## Biggest takeaways for a downstream auto-release system

1. **Trust `.release-please-manifest.json` (currently `1.1.0`), `package.json#version`, and the newest `v*` tag as the single source of truth** — they are consistent with each other; do NOT trust `HEAD`. Expect 0-N untagged commits on `master` between releases.
2. **Reuse the existing contract rather than invent a new one:** wire into `redtidev1918/releasegraph/.github/workflows/reusable-release.yml` (pinned SHA) as this repo does, or drive release-please with `include-v-in-tag` & `skip-github-release` to bump `package.json` + manifest. Version bumps flow to `v<semver>` tags and GHCR images.
3. **The real deliverable is the GHCR container image** `ghcr.io/redtidev1918/pixivflow-webui` (Dockerfile → nginx + `/api`,`/socket.io` reverse proxy). The GitHub-container being the canonical artifact keeps frontend↔backend versions implicitly coupled to whatever the deployer points `UPSTREAM_API` at — so pin both image tag and backend version together.
4. **Trigger on `push: master` + `workflow_dispatch`/`release` events, always with `dry_run` for PRs**, and run release-pipeline **after** the branch-contract gate (`reusable-branch-contract.yml`) so ops/release/hotfix branches stay clean.
5. **Compensate for the coupling gaps:** no backend-version lock, no frontend→backend version check, and a same-origin `/api`+`/socket.io` requirement mean the auto-release system should (a) co-version frontend and backend, (b) verify image cross-build (`docker buildx imagetools inspect` already in the policy), and (c) never assume an unknown `dist/` layout or a runtime configurable in-place API base (it's build-time baked).