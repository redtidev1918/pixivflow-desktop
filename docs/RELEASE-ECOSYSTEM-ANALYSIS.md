# PixivFlow 生态自动发布体系 — 阶段一：现状分析与目标设计

> 范围：仅分析与设计，**不改动任何仓库代码**。
> 结论先行：本生态已经由 `redtidev1918/releasegraph` 作为统一的发布「事务权威」管理，
> PixivFlow 与 pixivflow-webui 都是其中的受管仓库。本阶段需要补的不是「另一套发布系统」，
> 而是 releasegraph 目前**缺失的一环**：把「发布顺序/健康门禁」依赖图扩展为
> 「内容/打包」依赖 —— 用 `desktop-manifest.json` 锁定组件版本，并在上游发版时自动为
> 下游（pixivflow-desktop）提出版本升级 PR。

---

## 0. 术语与两义性（先厘清，避免设计错位）

本需求里出现的「依赖」实际是**两种不同的东西**，必须分开：

| 维度 | releasegraph 现有的 `dependsOn` | 本需求要的「组件依赖」 |
|---|---|---|
| 语义 | **发布顺序 / 健康门禁**（下游等上游 healthy 才 release） | **内容 / 打包依赖**（下游把上游产物打包进自己） |
| 数据 | release-graph.yml 里的边 + condition（healthy/complete） | 组件版本锁（如 desktop-manifest.json 里的 pin） |
| 动作 | 阻塞/放行 downstream 的发布节点 | 上游发版 → 下游 bump pin → 重建发布 |
| 目标 | 全 fleet 有序、可回滚地发版 | 上游新版本自动传导到成品仓库 |

**releasegraph 的现行规则（docs/concepts.md）** 对此有硬约束，直接决定了设计：

> - `Event → Reconcile → Plan → Dispatch → Exit`
> - 上游发布完成**只会唤醒下游重新计算**。下游**没有新的 desired version 时必须 NOOP**，
>   不能自动制造 patch version。
> - 依赖图必须无环；cycle / 缺失节点 / 非法 condition 一律 hard fail。
> - tag 不可移动；已有 tag 指向错误 commit 是 `TAG_CONFLICT`，必须停下问人。

含义：**releasegraph 天生拒绝「自动给下游 bump 版本」**。所以我们的自动升级不能由
releasegraph 凭空制造版本号，而必须通过一个**显式的「期望状态」载体** —— `desktop-manifest.json`
（desired-state lock）。bump manifest = 改变下游 desired 状态；PR 只是把这个改变提交通知人，
合并后下游才随之发版。这与「不能自动制造 patch version、不能替人 merge」完全兼容。

---

## 1. 当前 releasegraph 能力分析

### 1.1 它是什么
releasegraph 是一个 Go CLI + GitHub Actions reusable 工作流的 **fleet 发布事务权威**（`AGENTS.md`）。
业务仓库自身**不写发布逻辑**，只携带：
- `.release-policy.yml` —— 该仓库的发布契约（kind、versioning、build/test、assets、registries、postRelease、retention…）
- `.github/workflows/release.yml` —— 仅一行，调用 releasegraph 的可复用工作流。

### 1.2 执行模型
```
Desired + Actual + Policy + Dependency Graph → ReleasePlan
Event → Reconcile → Plan → Dispatch → Exit
```
- **Desired vs Actual**：事件不是事实源。收到事件后重新查询 git tag / GitHub Release / 资产 /
  workflow / registry，再生成计划（`docs/concepts.md`）。
- **Provider 状态是推导的，不是权威**：真实 GitHub/registry 状态决定健康；release-please 标签只是确认。
- **两套 scope**：`repository scope`（绑 `GITHUB_TOKEN`，单仓库）vs `fleet scope`（控制面，
  必须 `RELEASEGRAPH_FLEET_TOKEN`）。跨仓库操作必须 fleet scope，缺 token 立即
  `FLEET_CREDENTIAL_REQUIRED`。

### 1.3 CLI 命令面（internal/cli/cli.go）
`fleet` · `branch-contract` · `pr-lifecycle` · `doctor` · `graph` · `audit` · `inspect` ·
`plan` · `provider` · `rollout` · `status` · `explain` · `agent-context` · `repair` ·
`apply-plan` · `init` · `migrate` · `static-check` · `version`。
退出码是文档化契约（Usage / TransientRetry / Broken / NeedsReview / Blocked），供自动化判断。

### 1.4 依赖图原语（已存在，**直接可复用**）
`internal/domain/model.go`：
```go
type Project struct {
    ID        string
    Kind      NodeKind      // release | deploy | reconcile-only
    Repo      Repository    // owner + name
    DependsOn []Dependency  // {ID, Condition: healthy|complete}
}
type ReleaseGraph struct {
    APIVersion string
    Owner      string
    Projects   map[string]Project
}
```
`internal/graph/graph.go` 提供 `Load`、`Validate`（apiVersion=releasegraph.dev/v1、判环、缺节点、非法 condition）、
以及 **View{Order, Ready, Blocked, Upstream, Downstream}**。示例见 `examples/control/release-graph.yml`：
```yaml
apiVersion: releasegraph.dev/v1
projects:
  core: {kind: release, repo: {owner: acme, name: core}}
  cli:  {kind: release, repo: {owner: acme, name: cli}, dependsOn: [{id: core, condition: healthy}]}
  web:  {kind: release, repo: {owner: acme, name: web}, dependsOn: [{id: core, condition: healthy}]}
  deploy: {kind: deploy, repo: {owner: acme, name: deploy},
           dependsOn: [{id: cli, condition: healthy}, {id: web, condition: healthy}]}
```
由 `releasegraph graph plan` / `releasegraph plan`（读 `release-graph.yml`，默认路径）
→ `reconcile.Inspect(client, graph)` 产出 `Plan{Ready, Blocked, Noop, Nodes[]NodePlan{... BlockedBy}}`。

> **关键差距（经审计验证，`docs/en/plan.md:62-67` 原文）**：这个图是**编排（orchestration-only）关系**，
> 明确「不会在上游发版后于下游仓库开 dependency-update PR」；`graph.go` 的 `Downstream()` 只返回
> **一跳**直连下游、无传递闭包、无组件→下游映射。它**没有**「某 product 锁了哪些组件版本」的表达，
> 也没有「开 manifest 升级 PR」的动作。`desktop-manifest.json` 与「下游升级原语」正是要补的。

### 1.5 版本管理 / 发布事务（provider + reusable 工作流）
- 版本号唯一事实源：**业务仓库 `package.json` 的 `version`**（PixivFlow `2.46.0`，webui `1.1.0`）。
  镜像到 `.release-please-manifest.json`（`{".":"2.46.0"}`），tag = `v<semver>`（`include-v-in-tag: true`）。
- desired 版本解析（`internal/policy/policy.go:485-522`）：显式 `--version` 优先 → manual/tag 模式读
  `versioning.version` → release-please 模式读 `.release-please-manifest.json`。版本模式是**封闭枚举**
  `["release-please","manual"]`（`schemas/release-policy.schema.json:14`），无其他 provider。
- **发布是被业务仓库 push 出来的（serverless，无轮询守护进程）**：`reusable-release.yml` 用
  `on: workflow_call`，由业务仓库 release.yml 在 push/schedule/manual 时调用；每日 `fleet-audit`/`pr-lifecycle`
  只 reconcile **健康**，永不替下游发布新版本。registry `.latest` 仅是只读仪表盘字段（`release_infra/inventory.py`），**不是触发源**。
- 流程：release-please **只生成发版 PR + CHANGELOG**（`skip-github-release: true`）；
  **真正发布**（npm/ghcr/GitHub Release/postRelease）由 releasegraph 可复用工作流
  `reusable-release.yml@<SHA 钉死>` 依据 `.release-policy.yml` 执行。
- 工作流触发：`workflow_dispatch`(version/dry_run/force/repair/stage) + push master + 每小时 cron +
  PR（PR 强制 dry_run）。`branch-contract.yml` 在 ops/release/hotfix 分支上做强门禁。
- **发布安全**：正式 Release 在测试/构建/资产检查后才公开；Draft 先行、资产闸门通过才提升；
  构建产物落到 draft；已有 tag 指向错误 commit、同名资产 digest 不同、immutable registry 冲突 → hard fail。
- **恢复**：基础设施故障用**同版本** `repair/retry` 恢复，绝不靠再 bump patch 绕过（AGENTS.md 规则 6/10）。

### 1.6 发版后动作（postRelease）
`release.postRelease[]`（`type: github-workflow` 目前唯一 adapter）：Release 审计通过后、由
ReleaseGraph 显式 dispatch 相应 workflow，并传 `{{tag}}`/`{{version}}`/`{{repo}}`/`{{release_id}}`/`{{release_url}}`。
幂等 + 可重试（`post_release_health`: absent/pending/satisfied）。
**对触发设计极其重要的一点**（来自 webui/backend 审计）：
`release: published` 事件**只对 PAT/App 创建的 Release 触发**，而 `GITHUB_TOKEN` 创建的 Release
不会级联触发 workflow —— 所以**下游自动升级必须挂 releasegraph 的 postRelease/dispatch 或
workflow_call 唤醒，而不是监听 `release: published` webhook**。这正是 concepts.md 里
「exact tag 显式下传，下游永远不需要猜 latest」的依据。

### 1.7 跨仓库「bump → 开 PR」机制（已存在的直接范本）
`releasegraph rollout plan/apply --version vX.Y.Z [--repos ...] --apply`（fleet scope）：
读取 `fleet.yaml`，解析目标版本到 commit，用 `rollout.RepinToCommit` 重钉仓库里对 ReleaseGraph
的调用，然后为每个未达标仓库**开升级 PR**（`chore(release-infra): bump ReleaseGraph to ...`），
并可回滚（`rollout rollback --to ... --from ...`）。默认 **plan/dry-run，`--apply` 才算真的开 PR**。
这 = 本设计「下游 manifest bump PR」的现成机械装置，应复用其 PR 生成路径，而不是另起炉灶。

### 1.8 当前状态快照（STATUS.md，fleetaudit 生成）
- PixivFlow：`managed`，Desired=Latest=`v2.46.0`，**HEALTHY**
- pixivflow-webui：`managed`，`v1.1.0`，**DEGRADED**（Assets=0；社区镜像 ghcr.io/redtidev1918/pixivflow-webui）
- releasegraph 自身：tag-promoted，Latest `v1.5.12`（policy 内 version 字段 1.4.8 与 tag 有漂移）
- pixivflow-desktop：**尚未入 fleet**

---

## 2. PixivFlow 生态依赖图

### 2.1 仓库与制品（现状）
| 仓库 | kind | 版本源 | tag | 发布制品 | registries |
|---|---|---|---|---|---|
| redtidev1918/PixivFlow | node-library | package.json `2.46.0` | `v2.46.0` | `pixivflow-*.tgz`（required）| npm(required) + ghcr(可选) + GitHub Release(required) |
| redtidev1918/pixivflow-webui | container | package.json `1.1.0` | `v1.1.0` | 无 binary（dist/） | ghcr(required) + GitHub Release |
| redtidev1918/pixivflow-desktop | (新) hybrid/desktop | package.json + desktop-manifest.json | `v<semver>` | `.exe .dmg .AppImage` + tauri updater | GitHub Release(required) |

### 2.2 内容依赖（谁把谁的产物打包进去）
- **PixivFlow(backend) 自含** `webui-frontend/dist`（预构建、随 npm 发布；`STATIC_PATH` 指定；
  缺失时 Docker 退化为对 `pixivflow-webui@HEAD` 的**非钉死 clone**）。
- **pixivflow-desktop 打包**：backend 的 `node dist/webui/index.js` 运行时 + 构建产物；
  以及 webui 的 `dist/`（作为 STATIC_PATH，或退化为用 backend 自带的 frontend）。
- webui 对 backend 是**纯 API 契约耦合**（`/api/*`、`/socket.io` 同源相对引用；构建期
  `VITE_API_BASE_URL`；`localStorage 'auth-storage'`），**没有版本锁**。

因此，生态依赖方向（目标架构与需求一致）：
```
releasegraph ──受管──▶ PixivFlow(backend) ─┐
                    └─▶ pixivflow-webui ───┼─▶ pixivflow-desktop(成品，锁两者版本)
```
发布门禁（release 顺序）与内容（打包）两个方向一致，但机制不同：
- 门禁：releasegraph 的 `dependsOn`（desktop 等 backend 与 webui 均 healthy）。
- 内容：`desktop-manifest.json` 锁定 backend & webui 的具体版本 pin。

### 2.3 反模式警示（现状问题）
- 后端 Docker 对 webui 的非钉死 clone → 不可复现的前端构建。
- 无任何 backend↔webui 版本锁 → 大版本不匹配时同源托管可能 API 不兼容。
- 系统内「发布顺序」有图，但「哪个版本进桌面包」靠人手 —— 这是本设计要消化的缺口。

---

## 3. 推荐目录结构

沿用三个既有仓库 + releasegraph 控制面，不新建微服务：

```
redtidev1918/releasegraph/            # 控制面（受管、tag-promoted，被动扩展）
  fleet.yaml                          # 增补 pixivflow-desktop（enabled: true）
  graphs/pixivflow.yml                # 新增：本生态依赖图（含 desktop 节点，desktop 在 fleet.yml 配置文件中，desktop 的 dependsOn 见内）
  schemas/desktop-manifest.schema.json  # 新增：manifest schema
  .github/workflows/fleet-downstream.yml # 新增：下游自动升级（fleet scope，复用 rollout 机制）
  internal/…（新增 primitive：downstream/downstream.go 下游升级计划）→ 不碰现有 stable workflow
  # 现有 reusable-release.yml / pr-lifecycle.yml / branch-contract.yml / fleet-audit.yml 保持不动

redtidev1918/PixivFlow/               # 不改（继续发布 npm + ghcr + release，postRelease 唤醒下游）
redtidev1918/pixivflow-webui/         # 不改（继续发布 ghcr + release）
redtidev1918/pixivflow-desktop/       # 目标工作仓库
  desktop-manifest.json               # 新增：组件版本锁（本设计核心）
  .release-policy.yml                 # 新增：调用现成 reusable-release.yml 的模式（与兄弟仓库同构）
  .github/workflows/release.yml       # 新增：一行调用 releasegraph reusable（同构）
  .github/workflows/branch-contract.yml
  src-tauri/resources/backend/        # 打包的 backend（按 manifest pin）
  src-tauri/resources/webui/          # 打包的 webui（按 manifest pin）
  scripts/bundle.mjs                  # bundle 时校验产物版本 == manifest pin
```

原则：**业务仓库零发布逻辑**（releasegraph 现有规则 9）；新增能力全部放进 releasegraph 控制面
或 desktop 仓库自身的 manifest/bundle，不侵入 PixivFlow / webui。

---

## 4. manifest 设计（desktop-manifest.json）

### 4.1 规范（推荐，兼容用户给出的简写形式）
```jsonc
{
  "$schema": "https://raw.githubusercontent.com/redtidev1918/releasegraph/main/schemas/desktop-manifest.schema.json",
  "apiVersion": "releasegraph.dev/v1",
  "product": "pixivflow-desktop",
  "manifestVersion": "1",
  "version": "1.0.0",                     // 桌面成品自身版本（与 package.json.version 一致）
  "components": {
    "pixivflow": {
      "version": "2.46.0",               // 目标编译/打包版本 pin（semver，不含 v）
      "source": "npm:pixivflow@2.46.0",  // 取得产物的精确渠道（钉死，杜绝 4.2 的非钉死 clone）
      "commit": "84078e3…",              // 期望 commit（与后端 AGENTS.md「钉 commit 而非 tag」一致）
      "minApiVersion": "2.34.0",         // webui 同源的 API 兼容下限（可选）
      "migration": "none"                // none|required|warn
    },
    "webui": {
      "version": "1.1.0",
      "source": "ghcr.io/redtidev1918/pixivflow-webui@…|dist tag 或 npm?" ,
      "commit": "43ad36e…"
    }
  },
  "gates": {                              // 下游升级闸门
    "autoMerge": "on-green",              // never | on-green | manual
    "requireBreakingAck": true
  }
}
```
> 纯简写形式 `{"components":{"pixivflow":"0.8.0","webui":"0.5.0"}}` 是合法子集：
> 版本字段做唯一必填，source/commit/minApiVersion/migration 全部可选。**建议直接存规范版**
> （钉 source+commit），简写仅作为手写容忍态。

### 4.2 manifest 语义（与 releasegraph 模型对齐）
- manifest 是 **desired-state lock**：`components.<name>.version` = 本轮桌面发布打算打进包里的
  组件版本。**bump manifest = 改变 desired 状态**，这正好是 releasegraph 判断「这次下游要不要动」
  的输入 —— 解决 concepts.md「不得自动制造 patch version」的约束（bump 必须是一个显式产物）。
- schema 新增进 releasegraph `schemas/`，并在 releasegraph entangle 里加 `releasegraph manifest-lock`
  之类的只读/apply 子命令；**独立分支可并行，依赖环 hard fail**（复用现有 graph.Validate）。

---

## 5. GitHub Actions 设计

### 5.1 既有稳定工作流：**一字不改**
`reusable-release.yml`、`pr-lifecycle.yml`、`branch-contract.yml`、`fleet-audit.yml`、
各业务仓库的 `release.yml` / `branch-contract.yml` —— 全部保持原样，只在**新仓库**里套用同一模式。

### 5.2 新增 A：pixivflow-desktop 的发布接入（照搬兄弟仓库同构写法）
`.github/workflows/release.yml`（PixivFlow 模板）：
```yaml
jobs:
  release:
    uses: redtidev1918/releasegraph/.github/workflows/reusable-release.yml@<当前钉死 SHA 不变>
    with:
      version: ${{ inputs.version || '' }}
      dry_run: ${{ github.event_name == 'pull_request' || inputs.dry_run || false }}
      force: ${{ inputs.force || false }}
      repair: ${{ github.event_name == 'pull_request' || inputs.repair || false }}
      stage: ${{ inputs.stage || 'all' }}
    secrets: inherit
```
`.release-policy.yml`：`kind: hybrid`，`versioning.mode: release-please`；
`build` 里先跑 `scripts/bundle.mjs`（校验产物版本 == manifest pin），再 Tauri build；
`assets.required: [PixivFlow.Setup.exe/pixivflow-desktop_x64…, *.dmg, *.AppImage]` + tauri 签名文件；
`registries.github.required: true`；`postRelease`：刷新下载页 + 触发 Tauri updater 的端点校验事件。

### 5.3 新增 B：下游自动升级工作流（fleet scope，全部新增，不触既有流程）
`releasegraph/.github/workflows/fleet-downstream.yml`（示意）：
- 触发：`workflow_dispatch` + `schedule`（兜底轮询）+ 可选的「上游 postRelease 唤醒」输入
  （上游发版事务收敛后，控制面给本工作流一个 `wake=tag` 事件，避免只靠轮询 —— 呼应
  `release:published` 不级联的问题）。
- 步骤（复用 rollout 的 plan/PR 机制，fleet scope + `RELEASEGRAPH_FLEET_TOKEN`）：
  1. 读 `graphs/pixivflow.yml` 依赖图 + 每个下游的 `desktop-manifest.json`；
  2. 对每个进阶组件，对比「当前 pin」与「provider 现已发布的最新稳定 version」
     （源自 GitHub Release/tag，**绝不凭空猜**；等价复用 `provider.Inspect` / `rollout.InspectPins`）；
  3. 若有更新 → `downstream plan --apply` 在目标下游开 PR：
     `chore(deps): bump pixivflow to 2.47.0`（先 plan/dry-run，`--apply` 才真开）；
  4. 依据 release-notes（Breaking changes / Upgrade notes）给 PR 打标：
     `desktop: migration-required`。
- **自动合并策略**：默认 `never`；配置 `on-green` 时仅当 (a) 目标下游 CI（bundle+test）全绿
  (b) 无 Breaking/migration 信号 (c) 非 `requireBreakingAck` —— 才可自动合并。
  任何 Breaking/migration 强制转人工。（呼应「自动发布未经测试版本」禁令。）

### 5.4 触发链路（避免 `release: published` 坑）
```
上游 Release 事务收敛（releasegraph postRelease/post_release_health=satisfied）
   │
   ├─(已有) wake：reconcile 唤醒下游重算 —— 现在只让 desktop 变成「有 active desired」才放行
   └─(新增) fleet-downstream.yml：扫描 manifest → 开 bump PR（fleet token）
         合并 → desktop 新的 desired 状态落地 → 其自身的 release.yml(reusable) 正常发版
```
轮询 cron 仅作兜底；主路径靠「上游发版 → 控制面唤醒 → bump PR」。这与 AGENTS.md
「event 不是事实源、reconcile 重新读真实状态」一致。

---

## 6. 自动升级流程（端到端）

### 场景 1：PixivFlow 发布 v2.47.0
1. backend 发版走现有链路（release-please PR → 合并 → reusable-release.yml 发 Release+npm+ghcr，HEALTHY）。
2. 控制面 `fleet-downstream.yml` 被唤醒/Poll：
   - 读 `graphs/pixivflow.yml`：发现 desktop `dependsOn pixivflow`；
   - 读 desktop `${components.pixivflow.version}` = `2.46.0`；
   - 发现已发布 `2.47.0` > pin → 生成下游升级 PR（dry-run 先行，然后 `--apply`）。
3. **升级 PR**：`update desktop-manifest.json`（pixivflow → 2.47.0）+ 由 CI 按新 pin 跑
   `scripts/bundle.mjs`（重打包 backend/webui 并校验产物与 pin 一致）+ Tauri build/test + 桌面端可用性冒烟。
4. **闸门**：required checks 全绿；无 Breaking 则按 `on-green/manual` 合并；有 Breaking/migration
   → 转人工 + PR 挂 `desktop: migration-required`。
5. 合并后：desktop 的 `desired` 改变 → 其 `release-please + reusable-release.yml` 产出桌面
   `vX.Y.Z`，发布 `.exe/.dmg/.AppImage` + tauri updater；postRelease 刷新下载页。

### 场景 2：pixivflow-webui 发布 v1.2.0
同一机制，只是模板组件是 `webui`：bump `components.webui.version` → 重打包 webui dist →
同闸门 → 合并 → 桌面发版。后端无需联动（除非 webui 依赖的 API 有 Breaking → 由
`minApiVersion` + release-notes 检测拦截并对桌面标 migration）。

### 场景 3（不做）：desktop 不应自动 bump
desktop 自身永远不会有「自动升版本」。它只跟随上游组件的显式 pin 变化。没有新 desired
（没有待合并与含 pin 变更），它就是 NOOP —— 这是 releasegraph 规则，也符合
「自动发布未经测试版本」的禁令。

---

## 7. 风险分析与对策

| 风险 | 说明 | 对策 |
|---|---|---|
| **Breaking change / 大版本不兼容** | backend 0.8→1.0 或 webui 断 API，同源托管下桌面崩溃 | release-notes Breaking/Upgrade 检测 → PR 标 migration-required → 强制人工；manifest `minApiVersion` 硬闸；desktop 启动时按需提示 migration |
| **非幂等 / 自动发布未经测试版本** | bump PR 即便合并，若 CI 未覆盖真打包产物 | required checks = 按新 pin 的真 bundle+build+冒烟测试；`autoMerge` 默认 never |
| **`release: published` 不触发** | GITHUB_TOKEN 建的 Release 不级联事件 | 用控制面 postRelease/唤醒 + 轮询兜底，绝不依赖 webhook |
| **tag 漂移 / TAG_CONFLICT** | 既有 tag 移动 | 全部交给 provider，绝不动 tag；repair/waive 走既有原语 |
| **前端产物不可复现** | 后端 Docker 对 webui 无 pin clone | desktop 用 manifest pin 的 source 取 webui dist + 产物 hash 校验，不落非钉死 clone |
| **回滚失败** | 新组件回归 | `releasegraph downstream rollback` 反向 bump（复用 rollout rollback）；不动历史 tag，重钉旧 pin 同一 commit |
| **依赖环 / 非法图** | 误配导致死锁 | 复用 graph.Validate hard fail + 独立分支并行（不阻塞无关节点） |
| **权限 / 越权** | 跨仓库操作误触 | fleet scope 强制 + `FLEET_CREDENTIAL_REQUIRED`；`--apply` 显式；下游 bump PR 不开 AutoPriority merge（除非策略放开） |
| **schema 漂移** | manifest 结构与策略不同步 | `compatibility.json` 的 workflow_api/policy_schema 版本化；migrate 命令驻留 releasegraph |

---

## 8. 分阶段实施计划（先设计验证，后落码）

- **Phase 0 — 接入现状（0 新机制）**：把 `pixivflow-desktop` 纳入 fleet（fleet.yaml enabled）；
  补齐 `package.json` 版本 + `.release-policy.yml` + `release.yml`（照搬兄弟仓库模板）+ `branch-contract.yml`。
  产出：桌面能够用现成 releasegraph 链路独立发版。可持续交付。
- **Phase 1 — 版本锁落地**：新增 `desktop-manifest.json` schema（进 releasegraph/schemas）+ desktop 根文件；
  `scripts/bundle.mjs` 把产物下载/构建改为「按 manifest pin + commit + hash」；bundle 自检 pin==产物版本。
  加 `graphs/pixivflow.yml` 生态图（desktop dependsOn backend+webui condition healthy）。
- **Phase 2 — 下游升级原语（核心增量）**：releasegraph 新增 `downstream plan/apply`（复用 rollout 的
  PR 生成路径）+ `.github/workflows/fleet-downstream.yml`；用**示例仓库/desktop dry-run** 打通
  「上游发版 → 生成 bump PR → CI 按新 pin 跑」。此阶段仍默认手动合并。
- **Phase 3 — 自动化闸门**：升级 PR required checks 固化（bundle+test+冒烟）；release-notes
  Breaking/migration 检测；`autoMerge: on-green` 策略（可配、默认 manual）；Tauri updater 发布产物
  + 签名 + 端点校验。
- **Phase 4 — 回滚与推广**：`downstream rollback` 打通；把同一 manifest 机制推广到其他「打包下游」
  （如 `pixivflow-telepost-deploy` 等），共享同一原语，不各自造轮子。

每阶段独立可合并且完成「现状→目标」的无缝过渡，不影响既有稳定 workflow。

---

## 9. 一句总结

**不要重写 releasegraph，也不要给 PixivFlow/webui 加发布代码。**
PixivFlow 生态已经把「发版事务」做得很好；缺的是「组件版本锁 + 自动下游升级 PR」这一环。
用 `desktop-manifest.json` 承载期望状态、复用 releasegraph 的依赖图（gate）与 rollout PR 机制
（bump），新增一个 fleet 级 `fleet-downstream` 工作流把它串起来，就得到一套
「上游发布 → 自动检测影响 → 自动生成下游升级 PR → 测试闸门 → 合并后自动发布桌面成品」的体系，
同时严格守住「不自动造版本、不替人合并、不移动 tag、自动发布前必过测试」的既有规则。

---

## 10. 附录：releasegraph 能力审计（A–H，file:line 佐证）

> 来源：阶段一定制子代理对 releasegraph 控制面的深度审计（完整报告另存
> `/Users/jubaofeng/Documents/Code/releasegraph_PHASE1-audit.md`）。以下为带文件定位的浓缩结论，
> 用于把第 1 节的定性判断固化为「已支持/缺失」的可核对清单。

- **A 版本追踪**：desired 版本从每仓库 manifest/tag 读，而非轮询（`internal/policy/policy.go:485-522`：
  `--version` 优先 → manual/tag 读 `versioning.version` → 否则读 `.release-please-manifest.json`）。
  版本模式为封闭枚举 `["release-please","manual"]`（`schemas/release-policy.schema.json:14`）。
  registry `.latest` 只是只读仪表盘字段（`release_infra/inventory.py:62-94,198-223`），**非触发**。
  新版本由业务仓库经 `reusable-release.yml`（`on: workflow_call`）在 push/schedule/manual 时 **push**，
  serverless、无守护进程（`README.en.md:17`）。
- **B 依赖/影响图**：声明式 DAG 存在但**仅编排**。`docs/en/plan.md:62-67` 原文：
  “The graph's dependsOn is a declared orchestration relationship, not a dependency-manifest
  analysis … it does not open dependency-update pull requests in downstream repositories”。
  `internal/graph/graph.go:147-183` 由 dependsOn 算 Upstream/Downstream/Blocked/Ready；`Downstream()`
  只返回**一跳**直连下游，无传递闭包、无组件→下游映射。节点 kind：release/deploy/reconcile-only。
- **C tag/发布原语**：一等支持 —— 可复用发布事务（`reusable-release.yml`）、provider
  repair/reconcile/acknowledge/waive（`internal/cli/provider.go:52,264-277`）、
  `TAG_CONFLICT`=绝不移动 tag、停下问人（`internal/provider/provider.go:156-159`）、
  waive 打 `releasegraph: historical-waived` 标签。tag 模板默认 `v{version}`（`policy.go:524-526`）。
- **D 发版 PR 生成**：**不存在**。`docs/en/plan.md:62-67` 明说这是「separate primitive or manual step」。
  `pr-lifecycle.yml` + `internal/prlifecycle` 只分类/归档/关闭旧 PR（RELEASE/KEEP_OPEN/ACTIVE/
  MERGE_READY/OBSOLETE/PARKED/ATTENTION），写集仅 ensure_issue/comment/close_pull_request
  （`docs/pr-lifecycle.md:65-73`）——**不写 PR、不 merge、不 delete_branch**。
  四对象模型（Issue=backlog，branch=workspace，PR=merge queue，release PR=publish queue）是政策
  （`AGENTS.md:55-65`）；发版 PR = release-please 的 publish-queue PR。
- **E manifest**：仅存在 release-please 的**单仓库版本 manifest**本（`policy.go:25,503,516-521`）。
  **无产品依赖锁 manifest**；`schemas/` 只有 release-policy/plan/health 三个 schema 且 policy
  `additionalProperties:false` 不可扩展；grep `desktop-manifest` 零命中。`desktop-manifest.json` 无对应物。
- **F 测试闸门 & 回滚**：测试优先闸门经 `build.test/build.command/checksums/registry verify`
  （`docs/en/POLICY.md:5-22`、`reusable-release.yml` build job）。回滚=**同版本 repair**
  （`docs/en/RECOVERY.md`、`internal/cli/repair.go`）或 waive；**无「回滚到上一版本」原语**（设计上 tag 不动）。
- **G fleet vs repo scope**：代码强制两 scope —— `GITHUB_TOKEN`=repo，`RELEASEGRAPH_FLEET_TOKEN`=fleet，
  无 token → `FLEET_CREDENTIAL_REQUIRED`（`AGENTS.md:69-76`、`internal/domain/scope.go:5-24`、
  `internal/github/bound.go:213-223`）。**跨仓库 `DispatchWorkflow` 原语已存在**
  （`Client.DispatchWorkflow`→`repos/{repo}/actions/workflows/{file}/dispatches`，
  `internal/github/mutate.go:198-214`；provider repair 已用）。pixivflow-desktop 当前**不在** `fleet.yaml`。
- **H 差距清单（新增原语，且不动现有 stable workflow）**
  - 已支持（直接构筑）：单仓库发布事务；desired 版本解析；声明式 DAG+plan；tag 冲突/repair/waive；
    PR 生命周期治理（四对象模型）；fleet 跨仓库 dispatch（严格 scope）；测试闸门；postRelease ACK 幂等；只读 plan/audit 模式。
  - 缺失（作为新增原语补）：① 产品依赖锁 manifest + schema；② 影响检测
    「上游新版本→下游 manifest bump」；③ **PR 撰写**（branch/manifest edit/commit/open PR）——releasegraph 现在完全没有；
    ④ 上游发版唤醒→自动升级 PR；⑤ 跨仓库向 pixivflow-desktop **写** manifest-branch PR（dispatch 已存在，写 PR 没有）；
    ⑥ 真正的 manifest 回滚到上一版本；⑦ 一个「下游 pinned < 上游 latest」健康原因；⑧ 把 pixivflow-desktop 加入 `fleet.yaml`。

**插件式落法（只增不碰）**：全部新逻辑放进 releasegraph（`AGENTS.md` 规则 9/规则 1「add the primitive」）；
新增 `desktop-manifest.json` 及其 schema（`schemas/`）；**用独立「dependency-of-record」模型**
（组件→源仓库，即 manifest.`source`，如 pixivflow→PixivFlow、webui→pixivflow-webui）做影响检测，
**不 overload `release-graph.yml` 的 dependsOn**（dependsOn 仅保留发布顺序门禁）；新增只读检测微工作流
（仿 `fleet-audit.yml`：对比上游最新 release/registry vs manifest pin，pinned<latest 时产出 ready/blocked/noop plan）；
新增 fleet-scope PR 撰写原语（复用 `internal/github` bound client + `DispatchWorkflow`：建分支→改
`desktop-manifest.json`→commit→在 desktop 开升级 PR，先过 `build.test`）；该 PR 是 **merge-queue PR**
（尊重四对象模型，不强制桌面发版）。合并后由 pixivflow-desktop 自身的标准 `release.yml` +
`reusable-release.yml`（不改）发布成品。只用 `RELEASEGRAPH_FLEET_TOKEN`，绝不降级回 GITHUB_TOKEN。
**第一步：把 pixivflow-desktop 加入 `fleet.yaml`。**