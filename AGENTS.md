# pixivflow-desktop Agent 入口

先确认任务范围、git status、当前分支与远端；修改使用独立 worktree，不覆盖未知改动。

## 默认读取规则

- 普通开发：本文件、涉及目录的 AGENTS.md、相关源码与测试；按下表读取细则中对应标题段落（先 `rg -n '^#' docs/agent-guide.md` 定位，再按行读取）。不得因按需读取而省略适用约束。
- 接续旧任务：先读 `.agent/STATUS.md`（若存在），再检索该任务的 TASKS/HANDOFF/DECISIONS/LESSONS；不默认加载全部历史。
- 跨仓接口或职责变更：读取 Deploy AGENTS.md、CONTRACT.md 与相关架构章节；只扫描涉及仓库。
- 生产变更或事故：另读 Deploy `docs/operations/current-state.md`、相关运维/交付计划；历史按日期、PR、故障检索，不默认读全量记录。
- 领域、安全、发布和 CI 门禁不因文档拆分而放宽。保留标签、数据与审计历史；不打印凭据；不盲重试不确定的外部写入。
- 复用现有领域服务和 ReleaseGraph，不新建平行状态库或发布引擎。非平凡新增能力先调查已有实现。
- 文档与证据冲突时核对运行事实并修正文档。报告分别说明本地验证、CI、提交/PR、发行、部署、运行验收；不把测试通过当作生产通过。

## 按任务加载细则

完整规则保存在 [Agent 细则](docs/agent-guide.md)，以下标题均为检索入口。修改对应领域前必须读取该段及其引用的相关约束。

- Project Identity
- Architecture Rules
- Tauri Responsibilities
- Window model
- Native sign-in (embedded)
- The app menu
- BackendManager Contract
- Frontend Rules
- Runtime Rules
- Configuration Compatibility
- Logging & diagnostics
- Development Rules
- Testing Requirements
- Current Roadmap
- Commit Rules
- Working alongside other agents
- Verification gates (do not skip)
- Release pipeline (releasegraph)
- Documentation site (docsite)
- Cross-repo sync (upstream follows downstream)
