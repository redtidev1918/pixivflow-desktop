# PixivFlow Desktop

<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="PixivFlow Desktop" width="128" />
</p>

**语言 / Language:** 中文 · [English](README.en.md)

> **基于 PixivFlow 的原生桌面版，支持 Windows / macOS / Linux。**

PixivFlow Desktop 是 [PixivFlow](https://github.com/redtidev1918/PixivFlow) 的官方桌面发行版：把 PixivFlow 运行时和它的 WebUI 一起打包成一个**双击即用**的原生应用。装完不用另外装 Node、也不用自己起服务——桌面端只负责启动、守护和打包本地运行时，**业务逻辑全部留在上游 PixivFlow**。

[完整文档](https://redtidev1918.github.io/pixivflow-desktop/)

[![GitHub license](https://img.shields.io/github/license/redtidev1918/pixivflow-desktop?style=flat)](LICENSE) [![GitHub release](https://img.shields.io/github/v/release/redtidev1918/pixivflow-desktop?style=flat)](https://github.com/redtidev1918/pixivflow-desktop/releases) [![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat)](https://github.com/redtidev1918/pixivflow-desktop/releases) [![Made with Tauri 2](https://img.shields.io/badge/Made%20with-Tauri%202-purple?style=flat)](https://tauri.app/) [![Docs](https://img.shields.io/badge/Docs-文档站点-6366f1?style=flat)](https://redtidev1918.github.io/pixivflow-desktop/)

## 下载

从 [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) 取对应平台的文件（最新版本与校验和见 [下载页](https://redtidev1918.github.io/pixivflow-desktop/#/download.md)）：

| 平台 | 文件 | 首次打开 |
| :-- | :-- | :-- |
| **macOS**（Apple Silicon） | `PixivFlow-Desktop-v<版本>-macos-arm64.dmg` | 未签名：在「访达」里**右键 → 打开**，或到「系统设置 → 隐私与安全性」点「仍要打开」 |
| **Windows** 10/11 | `PixivFlow-Desktop-v<版本>-windows-x64-setup.exe` | 未签名：SmartScreen 里点「更多信息 → 仍要运行」 |
| **Linux**（x64） | `PixivFlow-Desktop-v<版本>-linux-amd64.deb` | 直接启动 |

每个 Release 同时提供 `SHA256SUMS` 与 `RELEASE-METADATA.json`：

```bash
sha256sum -c SHA256SUMS
```

安装包**未做代码签名**，所以系统会拦一次——这是已知限制，不是安装包损坏。目前 macOS 只产出 Apple Silicon 版 `.dmg`（原因见 [发布](#发布)）。

## 功能

- **双击即用** — 打开应用会自动读取配置、启动自带的 PixivFlow 后端、等健康检查通过后打开 WebUI；应用自带 Node 运行时，**用户机器上不需要装任何东西**。
- **窗口就是 WebUI** — 应用直接打开 PixivFlow 界面；管理面板只在后端起不来时出现（菜单栏随时可开 *Open manager* / *Open logs* / *Collect diagnostics* / *Cancel sign-in*）。
- **应用内登录** — Pixiv 授权页嵌在 WebUI 窗口里作为浮层打开：不开第二个窗口、不跳外部浏览器，登录结果直接回到 WebUI。
- **本地后端生命周期管理** — Rust `BackendManager` 负责启动、**接管**（崩溃后残留的后端进程）、优雅停止（关窗与退出应用时 `SIGTERM`）与重启，并按 **bundle → config → PATH → dev mock** 的优先级发现运行时。
- **自带运行时** — 安装包里带完整的 PixivFlow 后端（`resources/runtime/pixivflow/`，由 `runtime-manifest.json` 描述）与 WebUI dist，因此普通用户不需要单独安装。
- **运行状态可见** — 端口 / PID / 健康状态实时显示，来自 `/api/health` 探测，并通过 `backend-status` 事件推送到界面。
- **配置与数据位置** — `desktop-config.json`（首次运行自动生成）控制后端端口、是否自动启动与命令覆盖；你的 PixivFlow 数据（`config/`、`data/`、`downloads/`）存放在**每用户应用数据目录**，不会跟着应用目录走。
- **跟随系统语言的界面 + 一键收集诊断** — 菜单栏、对话框与兜底面板跟随系统语言（中文 / English）；**收集诊断 / Collect diagnostics** 会把日志、上次运行记录、原生崩溃报告、doctor 输出和一份**不含密钥**的配置打包到一个目录并打开文件管理器——这是反馈问题的推荐方式。

## 快速开始

### 用安装包

1. 从 [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) 下载对应平台的安装包并安装（首次打开按上表放行）。
2. 启动 **PixivFlow Desktop**：它会自动拉起本地后端并打开 WebUI。
3. 在窗口内完成一次 Pixiv 登录，之后数据与配置都留在本机。

### 从源码运行

需要 Node 20+、Rust 工具链和 [Tauri 前置依赖](https://tauri.app/start/prerequisites/)：

```bash
npm install              # Vite + @tauri-apps/*（前端与 Tauri CLI）
cd src-tauri && cargo fetch && cd ..

npm run tauri dev        # 开发运行：Vite dev server + 调试窗口
npm test                 # 前端单测 + 版本一致性 + dist 冒烟检查
cd src-tauri && cargo test   # 后端生命周期集成测试
```

把**真实的** PixivFlow 运行时装进 bundle（未提交的构建产物）：

```bash
npm run runtime:fetch    # 默认读兄弟目录 ../redtidev1918/PixivFlow
npm run webui:fetch      # 合成真实 WebUI 到 src-tauri/resources/webui/dist
```

仓库里提交的默认运行时是轻量 `dev-backend.mjs` 替身，所以全新 clone 不跑上面两步也能直接启动。更多细节见 [开发指南](docs/DEVELOPMENT.md)。

## 架构

```
PixivFlow Desktop (Tauri 2 + Rust)
        |
        v
PixivFlow 后端（上游，全部业务逻辑）
        |
        v
PixivFlow WebUI（上游 pixivflow-webui，桌面端直接打开）
```

三层各管一件事：**Desktop**（本仓库）只管运行时的打包、启动、守护与诊断；**Backend**（上游 PixivFlow）承担全部业务逻辑；**WebUI**（上游 pixivflow-webui）是桌面端打开的产品界面。桌面端**不复制**前端、**不实现**业务规则。详见 [架构说明](docs/ARCHITECTURE.md)。

## 为什么用 Tauri 2

- **包小** — 用系统原生 WebView，安装包是几十 MB 级，而不是 Chromium 的量级。
- **内存低** — 复用操作系统自带的 Web 引擎。
- **三平台一套代码** — Windows / macOS / Linux 同一份 Rust + 前端代码。
- **Rust 管进程** — 启动 / 停止 / 健康检查这类生命周期逻辑放在一个小而可审计的原生二进制里。
- **发布链路现成** — 安装包直接产出 GitHub Release 资产，由舰队引擎统一构建与校验。

## 与生态的关系

```
PixivFlow  ──►  pixivflow-webui  ──►  pixivflow-desktop
                        │
                        └── releasegraph 统一管理版本与发布
```

[releasegraph](https://github.com/redtidev1918/releasegraph) 统一管理生态内各仓库的版本关系与发布流水线。本仓库的 [`desktop-manifest.json`](desktop-manifest.json) 是**组件版本锁**（当前 `pixivflow` / `pixivflow-webui` 各锁一个版本），构建时按它取上游产物；未来也由它驱动桌面的自动升级。本仓库在发布图中的位置与依赖边见 [生态关系](docs/RELATIONSHIP.md)。

## 开发

| 命令 | 作用 |
| :-- | :-- |
| `npm run tauri dev` | 开发运行桌面应用 |
| `npm run build` | 构建前端 + `postbuild` 冒烟检查 |
| `npm test` | 前端单测 + 版本一致性 + dist 检查 |
| `npm run test:version` | 校验三处版本号一致（`package.json` / `tauri.conf.json` / `Cargo.toml`） |
| `cd src-tauri && cargo test` | 后端生命周期集成测试 |
| `bash scripts/build-release` | 本地跑一遍发布构建，产物落在 `dist/release/` |

改代码前请先读 [AGENTS.md](AGENTS.md)：它写明了本仓库的边界（不做业务逻辑、不引入第二个前端、不做自动更新）以及提交与发布约定。

## 发布

本仓库是 releasegraph 的一个节点，**发版全自动**：`main` 上的 Conventional Commit 会让 release-please 开一个 `chore(main): release x.y.z` 的 PR，**合并这个 PR 就是发布**——舰队引擎负责打 tag、跑三平台构建、校验资产名、生成 Release 与中文更新说明。没有人手动改版本号或建 tag；开一个 Pull Request 就等同于一次完整 dry run。

每次发布产出固定命名的安装包：

```
PixivFlow-Desktop-v<版本>-macos-arm64.dmg
PixivFlow-Desktop-v<版本>-windows-x64-setup.exe
PixivFlow-Desktop-v<版本>-linux-amd64.deb        （AppImage 视 runner 能力可选）
```

发布完成后，本仓库的 [文档站](https://redtidev1918.github.io/pixivflow-desktop/) 会自动刷新下载页并重新部署。

**已知限制（写清楚，不粉饰）**：

- 安装包**未签名**，首次打开需要手动放行。
- macOS 目前只有 **arm64** 的 `.dmg`：`macos-latest` runner 是 Apple Silicon，而随包携带的 `node` 用的是 runner 自己的那份；要出 Intel 包需要先把运行时改成按平台分发的发布产物（路线图 F4.2 / F4.3）。
- 尚无自动更新（路线图 F5）。

细节与故障排查见 [发布与版本管理](docs/RELEASE.md)。

## 项目状态

已经走完：**F0** 仓库骨架 → **F1 / F1.1** 可运行的桌面外壳与启动体验 → **F2.1** 真实后端适配 / 发现 / doctor → **F2.2** 打包运行时 + 方案 A 的 WebUI（后端用 `STATIC_PATH` 提供前端）→ **F2.3** 正式运行时契约与获取脚本 → **F4.0 / F4.1** macOS `.app` 打包闭环、真实运行时与真实 WebUI 一起进 bundle → **F4.4** CI 三平台发布矩阵（macOS / Windows / Linux 全部构建成功，`v0.2.0` 起为自动发布）。

还在前面：**F4.2 / F4.3** 按平台发布运行时产物并让桌面端自行下载/切换运行时（Intel `.dmg` 也依赖它）→ **F4.5** 命令行与命名收口 → **F4.6** `doctor` 自检命令 → **F5** 自动更新与一键部署。完整阶段计划见 [路线图](docs/ROADMAP.md)。

## 文档

仓库内的文档就是[文档站](https://redtidev1918.github.io/pixivflow-desktop/)的源文件：

| 文档 | 内容 |
| :-- | :-- |
| [快速开始](docs/QUICKSTART.md) | 安装、首次启动、登录、常见动作 |
| [架构说明](docs/ARCHITECTURE.md) | 三层边界、后端生命周期、运行时发现、方案 A |
| [开发指南](docs/DEVELOPMENT.md) | 环境准备、命令、目录结构、调试 |
| [发布与版本管理](docs/RELEASE.md) | 发版流程、版本号三处一致性、资产契约、已知限制 |
| [路线图](docs/ROADMAP.md) | F0 → F5 的阶段与验收标准 |
| [生态关系](docs/RELATIONSHIP.md) | 与 PixivFlow / pixivflow-webui / releasegraph 的依赖与版本锁 |
| [故障排查](docs/TROUBLESHOOTING.md) | 启动失败、端口占用、白屏、登录异常、日志与诊断包 |

## 贡献

见 [CONTRIBUTING.md](CONTRIBUTING.md)。安全问题请走 [SECURITY.md](SECURITY.md)，社区准则见 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

## 许可

MIT © [redtidev1918](https://github.com/redtidev1918)。上游 [PixivFlow](https://github.com/redtidev1918/PixivFlow) 与 [pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) 各自携带自己的许可证。
