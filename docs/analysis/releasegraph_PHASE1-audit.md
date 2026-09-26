# ReleaseGraph — Phase-1 Capability Audit for a Downstream Auto-Release System

Audited repo: `redtidev1918/releasegraph` (read-only) at `/Users/jubaofeng/Documents/Code/releasegraph`
Goal context: when a dependency (PixivFlow backend, pixivflow-webui) publishes a new version, automatically detect impact and generate an update-PR + release in `pixivflow-desktop`, driven by a version-locked manifest (`desktop-manifest.json` with `{components:{pixivflow, webui}}`).
Constraint: do NOT rewrite releasegraph; do NOT modify its stable workflows.

All evidence is `path:line`. Front-matter reading note: the docs are bilingual; Chinese (`.md`) and English (`docs/en/*.md`) are mirrors. I cite the English docs where both exist.

---

## A. Version tracking

### How releasegraph knows a repo's "desired/current" version
- **Desired version** is read from a per-repo manifest, not from a polled registry or ephemeral file:
  - `internal/policy/policy.go:485-522` — `DesiredVersion()` resolves the desired version:
    - explicit `--version` wins (`:486-491`);
    - `mode == "manual" || mode == "tag"` requires `versioning.version` (`:497-501`);
    - otherwise reads the **release-please manifest** `versioning.manifest` (default `.release-please-manifest.json`, `:503`) and picks the package entry (`DesiredVersionFromManifest`, `:511-522`).
  - `internal/policy/policy.go:21-25` — `Versioning{Mode, Provider, Manifest, Package, Version}` struct.
- **Version-provider modes** are a *closed enum*: `schemas/release-policy.schema.json:14` — `"mode": {"enum": ["release-please", "manual"]}`. These are the only expression of "versioning provider" in the policy schema.
  - `internal/policy/policy.go:493-496` — `Provider` is accepted as a fallback alias for `Mode` (release-please / manual / tag).
- **Provider-side kind taxonomy** (`internal/provider/provider.go:16-23`): `KindReleasePlease`, `KindTag`, and `State` enum (`StatePending/Triggered/Tagged/None/Unknown`, `:26-38`). These describe *acknowledgement* of a tag/release (release-please `autorelease: pending/triggered/tagged` labels; `releaseplease.go:45-51`), **not** independent version sources.
- **Registry "latest" detection** exists in the *fleet inventory* only (read-only dashboard), not as a release trigger:
  - `release_infra/inventory.py:62-94` — `_registry()` queries npm/pypi/pub `.latest` to report a *declared vs published* version in `STATUS.md`/`status.json`.
  - `release_infra/inventory.py:198-223` — GitHub Releases → `latest`, plus `.release-please-manifest.json` → `desired`; `status.json:3` carries `schema_version: 1`.

### How new releases are triggered — **business-repo workflow, never fleet polling**
- The release executes entirely inside the *business repository*, invoked via a reusable workflow:
  - `.github/workflows/reusable-release.yml:3-4` — `on: workflow_call` (a callable; callers pin it as `@v1` or commit SHA; `docs/callers.md:5-13`).
  - Triggered by standard GitHub events on the business repo: **push (release-PR merge)**, **schedule (recovery)**, **manual repair/force/dry-run**, `docs/en/RECOVERY.md:13-25`.
- **No polling daemon / no server / no central scheduler**: `README.en.md:17`, `docs/en/ARCHITECTURE.md:5-19`.
- A *scheduled* reconcile exists only in the **control repo** for *fleet health/recovery*: `.github/workflows/fleet-audit.yml:4-5` (daily), `.github/workflows/pr-lifecycle.yml:25-26` (daily). These reconcile *health*, they do not invent new downstream versions.

**Takeaway for A:** releasegraph knows a repo's *desired* version from a manifest/tag and *current* from live Git/Release/registry state; new releases are **pushed from the business repo**, never discovered fleet-side and published.

---

## B. Dependency / impact graph

### There IS a declarative DAG — but it is an *orchestration* graph, not a dependency-manifest/impact graph
- Graph model:
  - `examples/control/release-graph.yml:1-21` — `apiVersion: releasegraph.dev/v1`, `projects[*] {repo:{owner,name}, kind, dependsOn:[{id, condition}]}`.
  - `internal/domain/model.go` + `internal/graph/graph.go:14-20` — `View{Order, Ready, Blocked, Upstream, Downstream}`. **`upstream`/`downstream` are computed from the declared `dependsOn` edges** (`graph.go:147-170`), i.e. the reverse of `dependsOn`.
- Conditions: `internal/domain/model.go` (`ConditionHealthy`, `ConditionComplete`); graph must be acyclic and validated (`internal/graph/graph.go:37-96`), topo-sorted (`TopSort`, `:98-145`), cycle/missing-node/duplicate hard-fail.
- Node kinds: `release`, `deploy`, `reconcile-only` — `docs/en/ARCHITECTURE.md:33-39`, `internal/graph/graph.go:63-66`.
- Plan semantics: `internal/plan/plan.go:18-47` — `ready` / `blocked` / `noop`; `noop` when already `HEALTHY` (never force a re-release).

### The closest model (quote this in the design)
`docs/en/plan.md:62-67`:
> "The graph's `dependsOn` is a **declared orchestration relationship**, not a dependency-manifest analysis. `plan --graph` does not read downstream `pubspec.yaml` / `package.json` version constraints, and it does not open dependency-update pull requests in downstream repositories after an upstream release. That is a separate primitive or manual step. It only answers which nodes may start now and which are blocked by upstream nodes."

### Transitive/affected-downstream concept
- `internal/graph/graph.go:172-183` — `Downstream(g, id)` returns **direct** dependents only (one hop over `dependsOn`). There is no transitive closure over a *build/component→dependent* mapping, no "component -> build lists -> which dependents", no package-manager lockfile parsing, no impact computation.
- grep for `dependent / downstream / impact / affected / edge / DAG` confirms: **no impact-graph layer** — only the orchestration DAG above.

**Takeaway for B:** the concept of "an upstream wakes a downstream to recompute" exists, but there is **no** mechanism that translates "upstream published a new version" into "which downstream's manifest must bump". That is genuinely MISSING.

---

## C. Tag & release primitives

- **Reusable release transaction** — `.github/workflows/reusable-release.yml` (a `workflow_call`, `:3-4`), cut into `release_please` (version + provider reconcile), `build-plan`, `build` (matrix), `finalize` (tag → draft → publish → audit → postRelease). Transaction sequence: `docs/en/ARCHITECTURE.md:41-48` (`PREPARE → TEST → BUILD → ASSET GATE → VERIFY/CREATE TAG → …`).
- **Provider repair** (same-version recovery): `internal/cli/provider.go:28,52,80`, and `internal/cli/repair.go:127` — "dispatched same-version repair". Also `releasegraph provider [inspect|reconcile|acknowledge|repair|waive]` (`:52`).
- **Provider reconcile / ACK**: `internal/provider/releaseplease.go:20-51`, `internal/cli/provider.go`; ACK idempotently flips release-please labels; runs from the reusable workflow finalize (`reusable-release.yml:557-559`) and `reusable-acknowledge.yml`.
- **Waive** (explicit human acceptance of a historical version): `internal/cli/provider.go:264-277` (`releasegraph provider waive --repo … --version X`), `releaseplease.go` `releasegraph: historical-waived` label (`:50-51`). AGENTS.md rule 6: "Never fabricate historical GitHub Releases … Use an explicit waiver."
- **TAG_CONFLICT** handling: never move an existing tag; `internal/provider/provider.go:156-159` (`tag points at %s, expected %s`); AGENTS.md rule 5: "Never move an existing release tag. A tag pointing at the wrong commit is `TAG_CONFLICT` → stop and ask a human." Tag creation/verify only when absent or at the expected commit (`docs/en/ARCHITECTURE.md:54`).
- Tag template: `internal/policy/policy.go:524-526` — `ReleaseTag()` (`tag.template`, default `v{version}`), schema `schemas/release-policy.schema.json:20`.
- Immutable-tag promotion on the dogfood repo: `.github/workflows/infra-release.yml:120-123` (`git tag -f v1` moving the *stable channel*, its own deliberate special case).

**Takeaway for C:** delivering an exact version's tag + GitHub Release at an expected commit, repairing it, and waiving historical versions are all first-class, tested primitives — safe to build on. There is **no** "create/update a *moving* tag for a *different* repo" primitive exposed to callers (cross-repo requires fleet scope).

---

## D. Release-PR generation

- **No generic "update dependency to version X" PR / manifest-bump PR exists.** The explicit gap is documented at `docs/en/plan.md:62-67` (quoted in section B): opening dependency-update PRs downstream is explicitly declared a "separate primitive or manual step".
- What DOES exist — ReleaseGraph owns governance over PRs, but **does not author dependency PRs**:
  - `.github/workflows/pr-lifecycle.yml` + `internal/prlifecycle/contract.go`, `plan.go` — classifies open PRs (`RELEASE / KEEP_OPEN / ACTIVE / MERGE_READY / OBSOLETE / PARKED / ATTENTION`; `docs/pr-lifecycle.md:27-35`). It **closes/archives stale PRs**; it never opens or merges.
  - The **four-object model** (Issue=backlog, branch=workspace, PR=merge queue, release PR=publish queue) is policy: `AGENTS.md:55-65` (rule 12). "Release pull request" in this codebase means release-please's **publish-queue PR**.
  - Release-please owns the **Release PR** (and its manifest), because managed policy requires `skip-github-release: true` while release-please "manages conventional commits, changelog, versions, manifest, and the Release PR only" — `docs/en/POLICY.md:24`.
  - The PR lifecycle treats release-please/dependabot branches and the `release-please--*` glob as exempt (`docs/pr-lifecycle.md:100-107`, schema `release-policy.schema.json:185`).
- **Mutation is deliberately absent**: the closed action set is only `ensure_issue`, `comment`, `close_pull_request` (`docs/pr-lifecycle.md:65-73`); no `merge`, no `delete_branch`.

**Takeaway for D:** releasegraph **does not generate** dependency-update PRs or bump manifests; it only *governs* PRs. The "open an update PR in pixivflow-desktop" capability must be a **new primitive/micro-workflow** — but it can re-use the fleet-scoped PR/issue/comments machinery (bound client, `internal/github/mutate.go`).

---

## E. Manifest

- The **only "manifest" concept** is release-please's own version manifest used to decide a single repo's desired version:
  - `.release-please-manifest.json`, referenced by `versioning.manifest` — `internal/policy/policy.go:25,503,516-521`; `release_infra/policy.py:231-236`; `release_infra/inventory.py:23,214-220`.
  - It is a `map[string]string` of **component→version for that one repo** (`internal/policy/policy.go:511-517`), i.e. release-please monorepo components; default package `"."` (`:516`).
- **No cross-repo/product dependency-lock manifest exists.** `docs/` has no manifest doc; `schemas/` has only `release-policy.schema.json`, `plan-v1.json`, `health-v1.json` — **no manifest schema**. grep for `desktop-manifest` / `pixivflow-desktop` across the repo: **zero hits**.
- `status.json`/`STATUS.md` are generated snapshots (GENERATED — DO NOT EDIT), not an input manifest (`STATUS.md:3-6`, `status.json:3`).

**Takeaway for E:** `desktop-manifest.json {components:{pixivflow, webui}}` has **no equivalent** in releasegraph. This must be a new artifact + schema owned by the downstream micro-workflow (releasegraph's policy schema has `additionalProperties:false` and only understands its own release-policy / release-please manifests).

---

## F. Testing gate & rollback

- **Testing gate**: hard gates via `build.test`, `build.command`, asset gate, checksums, registry verification:
  - `docs/en/POLICY.md:5-22` — `build.test`/`build.command` run before release; required assets are exact contracts, hard gates (`docs/en/ARCHITECTURE.md:50`).
  - `.github/workflows/reusable-release.yml` `build` job runs the test/build matrix and a `version_check` binary gate (`:200-263`), and the asset gate in `finalize`.
- **Rollback / repair / waive**:
  - **Same-version repair** (recover an interrupted transaction at the SAME version) — never a new version, never a tag move: `docs/en/RECOVERY.md:3-4,9,29-40`; `internal/cli/repair.go`.
  - The four `release_health` outcomes (`healthy | tag-drift | repair | missing`) are a frozen contract (`docs/en/health.md:104-108`, `docs/callers.md:19`).
  - **Waive** for historical versions as-is — `internal/cli/provider.go:264-277`.
  - There is **no "roll back to previous version"** primitive that moves a tag backwards; rollback = re-run repair on an already-shipped tag `vX` at the correct commit, or a `waive`.
  - Recovery vs deterministic failure: `docs/en/ARCHITECTURE.md:52-54`, `docs/en/RECOVERY.md:38-40` (network retries; policy/graph/checksum/tag/registry conflicts do not retry).

**Takeaway for F:** test-first gating and same-version repair/waive are strong, reusable. True "rollback to an older version" does not exist (by design); a downstream manifest bump rollback would be a *new* concept.

---

## G. Fleet vs repository scope (cross-repo)

- Two-scope model, enforced in code:
  - `AGENTS.md:69-72` — `GITHUB_TOKEN` = repository scope (bound to `GITHUB_REPOSITORY`); `RELEASEGRAPH_FLEET_TOKEN` = fleet scope (control plane). Fleet without the fleet token fails fast with `FLEET_CREDENTIAL_REQUIRED` (`internal/credential/credential.go:4`, `AGENTS.md:74-76`).
  - `internal/domain/scope.go:5-24` — `ScopeFleet` may inspect/dispatch cross-repo; `ScopeRepository` refuses cross-repo writes.
  - `internal/github/bound.go:213-223` — `DispatchWorkflow` fails with `SCOPE_VIOLATION` in repo scope; allowed in fleet scope (tests `bound_test.go:65-95` assert this).
  - `internal/cli/provider.go:115-119` — cross-repo provider commands require fleet scope; `internal/cli/status.go:562` — "cross-repo mutation in repository scope" is forbidden.
- **Cross-repo dispatch already exists as a primitive** and is what "trigger px working in a repo that is not the caller" will use:
  - `internal/github/mutate.go:198-214` — `Client.DispatchWorkflow` → `repos/{repo}/actions/workflows/{file}/dispatches`.
  - Used today by `provider repair` to dispatch a same-version repair through a downstream repo's pipeline (`internal/provider/releaseplease.go:737`).
- Fleet inventory authority: `fleet.yaml` is the only source of truth; `fleet discover` is candidate-only (`fleet.yaml:1-8`, `AGENTS.md:38-39`). **`pixivflow-desktop` is not in `fleet.yaml`** currently (fleet.yaml lists PixivFlow, pixivflow-webui, pixivflow-telepost-deploy, but NOT pixivflow-desktop) — see `consolidation` table below.
- Deployment of a new ReleaseGraph engine across the fleet is `rollout` (`internal/rollout/rollout.go`, `fleet-rollout.yml`) — this is ReleaseGraph's own version upgrade, unrelated to dependency updates.

**Takeaway for G:** cross-repo *dispatch* is supported (fleet scope). What is missing is the *decision* step: read the upstream's new version, decide pixivflow-desktop is affected, and author the manifest bump + PR. The cross-repo write of a PR (commit/branch/PR) into pixivflow-desktop would require a new primitive using the fleet token.

---

## H. Gap analysis

### ALREADY SUPPORTED — build on it
1. **Single-repo release transaction** (tag + GitHub Release + assets + checksums + registry + audit) — `reusable-release.yml`, `internal/provider`, `internal/github/mutate.go`.
2. **Desired-version resolution** from a manifest/tag — `internal/policy/policy.go:485-522` (but tied to release-please/`.release-please-manifest.json`).
3. **Declarative DAG** with up/downstream, ready/blocked/noop, topo sort, cycle detection — `internal/graph/graph.go`, `internal/plan/plan.go`, `docs/en/plan.md`.
4. **Tag conflict + repair + waive** primitives (never move, repair same version, waive historical) — `internal/provider`, `internal/cli/repair.go`, `internal/cli/provider.go:264`.
5. **PR-lifecycle governance** (classify/archive/close, four-object model, exempt lists) — `pr-lifecycle.yml`, `internal/prlifecycle`, `AGENTS.md:55-65`.
6. **Fleet-scoped cross-repo dispatch** with strict scope enforcement — `internal/github/bound.go:213`, `internal/github/mutate.go:198`, requires `RELEASEGRAPH_FLEET_TOKEN`.
7. **Test-first gating** and health/reason vocabulary — `docs/en/POLICY.md:5-22`, `docs/en/health.md`.
8. **postrelease dispatch / ACK** idempotency — `release.postRelease` (`release_infra/actions.py`), `reusable-acknowledge.yml`.
9. **Read-only planning/audit** as a control-repo pattern — `reusable-readonly-plan.yml`, `fleet-audit.yml`.

### MISSING — must add as new primitives/micro-workflows WITHOUT touching stable workflows
1. **Dependency-version-lock manifest for a product** (`desktop-manifest.json`) + schema — nothing analogous exists (E).
2. **Impact detection: upstream new-version → which downstream manifest must bump** — the graph is orchestration-only; no transitive dependent/component mapping; explicitly documented absent (`docs/en/plan.md:62-67`).
3. **Authoring a dependency-update PR** (create branch, edit manifest, commit, open PR) — releasegraph deliberately has NO PR-authoring primitive (D); only governance.
4. **Trigger/"wake" mechanism that consumes an upstream release event** — events wake *recompute*, they do not auto-open downstream update PRs (B).
5. **Cross-repo *PR write* into pixivflow-desktop** — dispatch exists, but writing manifest branch/PR into another repo requires a new fleet-scoped capability (G).
6. **True rollback-to-previous-version** for a downstream manifest pin — only same-version repair/waive exist (F).
7. **A "desired downstream version is behind upstream" health/reason** — health today compares desired vs actual for ONE repo, not "downstream pinned < upstream latest" (A/F).
8. **`pixivflow-desktop` is not in `fleet.yaml`** — before any orchestration/DAG/dispatch can reference it, it must be declared (G).

---

## How pixivflow-desktop should plug in (consistent with NOT rewriting releasegraph)

Follow the existing serverless control-plane pattern and keep every stable workflow untouched:

1. **Put all new logic in releasegraph itself** (per `AGENTS.md:9` — "Do not put release logic in business repositories"). Add flight-side primitives (satisfies AGENTS.md rule 1: "If a primitive is missing, add the primitive to ReleaseGraph, test it, dry-run it, then use it").

2. **New manifest + schema.** Add `desktop-manifest.json` (`{components:{pixivflow, webui}}`) plus a JSON schema in `schemas/`, owned by a new micro-workflow. This is a self-contained artifact; it does not collide with `release-policy.schema.json` (`additionalProperties:false`) which stays untouched.

3. **New "dependency-impact" model separate from the orchestration DAG.** Do NOT overload the existing `release-graph.yml`/`dependsOn`. Add a dedicated `desktop-deps` declaration (component→source repo, e.g. `pixivflow → redtidev1918/PixivFlow`, `webui → redtidev1918/pixivflow-webui`) as a *dependency-of-record* input for the new micro-workflow. The existing graph module's `Downstream`/topo utilities can be reused as pure helpers, but read it as an independent manifest.

4. **New upstream-trigger detection (polled, fleet-side, read-only).** A new scheduled `workflow_dispatch`/`schedule` micro-workflow in the control repo (pattern: `fleet-audit.yml`) uses the read-only fleet token to compare each upstream's latest GitHub Release / registry version (reuse the `_registry`/release-read logic) against the pinned version in `desktop-manifest.json`. When pinned < upstream latest → plan a bump (NOOP/blocked otherwise), mirroring the `ready/blocked/noop` plan shape.

5. **New "open dependency-update PR" primitive (fleet scope).** Reuse `internal/github`'s bound fleet client + `DispatchWorkflow`/mutate plumbing to author a branch, edit `desktop-manifest.json`, commit, and open the update PR in `pixivflow-desktop`. Keep it green-gated: the existing release pipeline (`reusable-release.yml`) already runs tests/build — run the same `build.test` first, and open the PR to let the desktop repo's own CI + the PR-lifecycle governance gate it. Respect the four-object model: the update PR is a *merge-queue* PR; it must not itself force the desktop *release*.

6. **Desktop release is a separate, ordinary repo release.** Once the update PR merges and `desktop-manifest.json` changes, the standard single-repo release transaction (`reusable-release.yml` invoked by pixivflow-desktop's own `release.yml`) publishes the desktop release. So the auto-release value-add is *the manifest-bump PR*, and ReleaseGraph's existing, untouched release pipeline does the rest.

7. **Scope/security.** The update-PR micro-workflow runs only in fleet scope with `RELEASEGRAPH_FLEET_TOKEN` (never falls back to `GITHUB_TOKEN`), and only touches repos declared as its dependency-of-record (`fleet.yaml` + the new desktop-deps list). Add `pixivflow-desktop` to `fleet.yaml` as the first declared dependent.

Net: everything new is additive (new manifest + new schema + new micro-workflow + one new fleet-scoped PR-authoring primitive); every existing stable workflow (`reusable-release.yml`, `pr-lifecycle.yml`, `fleet-audit.yml`, `infra-release.yml`, `go-canary.yml`) is left untouched.