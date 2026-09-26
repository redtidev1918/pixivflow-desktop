# PixivFlow Desktop — 架构分析与集成方案

> 阶段 1 输出。基于对两个上游仓库的只读分析（不修改任何上游代码）。
>
> - Backend: `redtidev1918/PixivFlow` v2.46.0
> - Frontend: `redtidev1918/pixivflow-webui` v1.1.0

---

## 1. 当前架构分析

### 1.1 PixivFlow（后端）

| 关注点 | 结论 | 出处 |
|---|---|---|
| 技术栈 | Node.js + TypeScript，Express 4 + Socket.IO，SQLite（内置 `node:sqlite`，≥ Node 22.13，WAL） | `package.json`, `src/storage/Database.ts` |
| 主要入口 | `main: dist/index.js`, `bin: { pixivflow: dist/index.js }` | `package.json` |
| CLI | `dist/index.js` 是无子命令时的 CLI 分发器（默认跑 scheduler 或 download）；长驻命令有 `web`/`webui`/`w`、`scheduler` 等 | `src/index.ts` |
| **API 服务入口** | **`node dist/webui/index.js`** —— 独立的 WebUI+API 服务器，与 CLI 分开 | `src/webui/index.ts`, `src/webui/server/server.ts` |
| API 框架/端口 | Express 4；默认端口 `PORTS.PROD_API = 3000`（`src/webui/ports.ts`）；`PORT`/`--port` 可覆盖；默认 host `localhost`（`HOST`/`--host` 覆盖） | `src/webui/index.ts` |
| 健康检查 | `GET /api/health`、`GET /health` → `{status:'ok'}`（auth 豁免） | `src/webui/server/server-routes.ts` |
| 路由挂载 | `/api/auth` `/api/config` `/api/download` `/api/stats` `/api/logs` `/api/files` `/api/scheduler` `/admin/*`，同端口 Socket.IO | `src/webui/server/server-routes.ts` |
| 认证 | Basic 可选（`WEBUI_USERNAME`/`WEBUI_PASSWORD`）；**绑定非回环 host 且无认证会拒绝启动**（fail-closed，`WEBUI_ALLOW_PUBLIC_NO_AUTH=true` 逃生舱）。回环 127.0.0.1/localhost 无认证即可 | `src/webui/server/server.ts`, `auth-middleware.ts` |
| 配置 | JSON 配置文件（无 dotenv）。`loadConfig()`：`$PIXIV_DOWNLOADER_CONFIG` → 智能检测（config 目录 / `~/pixivflow/config`）→ 自动生成默认配置（自举） | `src/config/loader.ts` |
| 环境变量覆盖 | `PIXIV_DATABASE_PATH` `PIXIV_DOWNLOAD_DIR` `PIXIV_ILLUSTRATION_DIR` `PIXIV_NOVEL_DIR` `PIXIV_LOG_LEVEL` `PIXIV_SCHEDULER_ENABLED` 等 | `src/config/environment.ts` |
| 数据目录 | `storage.databasePath` 默认 `./data/pixiv-downloader.db`，`downloadDirectory` 默认 `./downloads`；相对路径相对配置文件/项目根解析；DB 启动自动建库迁移（自举，无需外部服务） | `src/config/defaults.ts`, `src/config/path-resolution.ts` |
| 日志 | 自定义 Logger：写 console + 文件；日志文件路径 = 绝对 `databasePath` 所在目录下的 `pixiv-downloader.log`（否则 `cwd/data/`）；`PIXIV_LOG_LEVEL` `PIXIV_LOG_FORMAT=json` `PIXIV_LOG_MAX_BYTES`(20MB) `PIXIV_LOG_RETENTION_DAYS`(30) | `src/logger.ts`, `src/webui/server/server.ts` |
| 生命周期 | `startWebUI()` 注册 `SIGINT`/`SIGTERM` → `server.stop()`（先关 Socket.IO 再关 HTTP）→ `process.exit(0)` **优雅关闭** | `src/webui/server/server.ts` |
| 静态前端托管 | 若有 `STATIC_PATH`（或相对目录 `webui-frontend/dist`）则同端口托管前端；`app.get('*')` 非 `/api` 路由回退 `index.html`（**已具备 SPA 回退**）；无前端也可纯 API 运行 | `src/webui/server/server-static.ts` |
| 外部服务 | 无 Redis/Postgres；仅 SQLite 自举 | `package.json deps` |

**关键结论：后端在 `127.0.0.1:<port>` 上能自举地同时提供「前端静态文件 + `/api` + `/socket.io`」三件套，并原生支持 SPA 回退与优雅停机。** 这是桌面集成可以做到“同源、零 CORS、零改动”的根因。

### 1.2 pixivflow-webui（前端）

| 关注点 | 结论 |
|---|---|
| 技术栈 | React 18 + TypeScript + Vite 7；antd5、react-query、zustand、react-router-dom (BrowserRouter)、socket.io-client、axios、i18next |
| 构建 | `npm run build` = `tsc && vite build` → `dist`（`base: './'` 相对基路径，纯静态 SPA） |
| API 地址 | axios base 由 **构建期** 注入的全局常量 `__VITE_API_BASE_URL__`（env `VITE_API_BASE_URL`，默认 `''` → 相对 `/api`）决定；`getApiBaseURL()` 另有运行时兜底 `globalThis.__VITE_ENV__.VITE_API_BASE_URL` |
| Socket.IO | **生产环境返回 `''`（同源相对路径）**——因此前端必须与后端同源，否则 WebSocket 连不上 |
| localhost 后端 | 开发：Vite proxy `/api`+`/socket.io` → `localhost:${VITE_DEV_API_PORT||3000}`。生产：默认依赖**同源相对 `/api`** |
| 认证 | 前端直连 PixivFlow API（`/auth/login`,`/auth/login-with-token`,`/auth/refresh`,`/auth/status`,`/auth/logout`），token 存 `localStorage`（key `auth-storage`），无 cookie |
| SPA 回退 | history 模式 BrowserRouter，需要服务器提供 `index.html` 回退才能深链刷新不 404 |
| 已备桌面钩子 | `src/types/electron.d.ts` 预留 `window.electron.openLoginWindow()` 逻辑（未启用），是后续登录优化的天然接入点 |
| 风险点 | 若干文件使用绝对 `window.location.href = '/dashboard'` / `'/login'`（全页刷新）与 `window.open('/api/files/preview?...','_blank')`；`window.location.reload()`。**只要由后端同源托管且 SPA 回退生效，这些都能正常工作** |

---

## 2. Desktop 集成方案

### 2.1 核心决策：同源托管（Embedded/本地模式）

```
PixivFlow Desktop (Tauri 2, Rust)
   │  1. 启动子进程:  node <backend>/dist/webui/index.js
   │       env: PORT / HOST=127.0.0.1 / PIXIV_DATABASE_PATH / PIXIV_DOWNLOAD_DIR /
   │            STATIC_PATH=<bundled webui dist> / PIXIV_LOG_LEVEL
   │     └─ 后端在 127.0.0.1:<port> 同端口提供: 前端静态 + /api + /socket.io
   │  2. 轮询 GET /api/health 直到就绪
   │  3. 创建主窗口 -> 加载 http://127.0.0.1:<port>  (WebviewUrl::External)
   │  4. 关闭窗口/退出 -> 发送 SIGTERM 优雅关闭后端
   │
   └─ 前端 pixivflow-webui 构建产物 由后端同源托管（无需改任何 webui 源码）
```

**为什么不需要修改上游 webui 源码：**
- 前端由后端同源托管 ⇒ 相对 `/api` 与相对 `/socket.io` 天然指向同一后端 ⇒ **无需构建期 `VITE_API_BASE_URL`，无需改 `client.ts`/`socket.ts`，无 CORS**。
- 后端 `app.get('*')` 已做 SPA 回退 ⇒ 深链刷新（`/dashboard` `/login` 的全页跳转）都能命中 `index.html`。

### 2.2 两种运行模式

| 模式 | 前端从哪来 | 主窗口 URL | 是否启动本地后端 |
|---|---|---|---|
| `local`（Embedded） | 后端通过 `STATIC_PATH` 托管 Tauri 打包的 webui `dist` | `http://127.0.0.1:<port>` | 是 |
| `remote`（Remote） | 复用远端 PixivFlow 服务器已托管的前端 | `<remoteUrl>` | 否（连接已有服务器） |

`remote` 模式不启动本地后端、不建本地 DB，直接加载用户提供的远端 WebUI，不破坏已有沿用 Docker/主机部署的用户。

### 2.3 BackendManager（Rust，trait 设计如上）

`start()` / `stop()` / `restart()` / `healthCheck()` 均由 Rust 维护；`stop()` 走 **SIGTERM → 等待退出 → 超时再强杀（非 `kill -9` 起步）**。

### 2.4 进程 / 端口 / 重复启动

- **重复启动桌面**：`tauri-plugin-single-instance`，二次启动聚焦已有窗口。
- **backend 已运行**：启动前先对 `127.0.0.1:<port>` 做 health；已健康则**接管**（不重复 spawn）；端口被非 PixivFlow 占用则报错。
- **backend 异常退出**：主进程监控子进程 `try_wait()`，退出时（非主动）回退到原始状态/错误提示。
- **端口占用**：由上述 health/占用检测与错误提示处理。

---

## 3. 需要修改 / 新增的文件

### 3.1 上游（原则：不修改，仅按需阅读；可选最小改动见下）
- **不需要改上游代码**即可打包桌面。若要“默认端口/默认数据目录”更贴合桌面，可在上游做可选微调，但桌面用环境变量覆盖，不必触碰：
  - `PixivFlow/src/webui/ports.ts`、`src/webui/index.ts`（端口/host 覆盖，桌面用 env 传参，无需改）
  - `PixivFlow/src/config/defaults.ts`、`src/config/path-resolution.ts`（数据目录，桌面用 `PIXIV_DATABASE_PATH`/`PIXIV_DOWNLOAD_DIR`，无需改）
  - pixivflow-webui：**无需修改**（同源托管路径）。

### 3.2 新增 `pixivflow-desktop` 仓库
```
pixivflow-desktop/
├── package.json                # 顶层脚本（tauri dev/build、bundle backend/webui）
├── README.md
├── docs/ARCHITECTURE.md
├── scripts/
│   └── bundle.mjs              # 构建并复制 PixivFlow dist + pixivflow-webui dist 进资源目录
├── src/frontend/index.html     # 极简启动回退页（Tauri frontendDist，目录对齐 F0 结构）
└── src-tauri/
    ├── Cargo.toml / build.rs / tauri.conf.json / capabilities/default.json / icons/
    ├── resources/backend/ ../webui/   # 打包时填充（gitignore，不入库）
    └── src/
        ├── main.rs             # builder、单实例、窗口生命周期、退出钩子
        ├── config.rs           # config.json 读写（mode/backend/data/logs）
        ├── backend.rs          # BackendManager start/stop/restart/health
        ├── health.rs           # 零依赖 HTTP 健康探测（TcpStream）
        ├── logging.rs          # 最小文件日志（logsDir）
        └── commands.rs         # Tauri commands（get/start/stop/restart/config）
```

> **F0 对齐（2026-09）**：`src-shell/` 已归入 `src/frontend/`；新增
> `assets/`（品牌源图，构建图标由 `npm run icon` 从 `assets/icon.png` 生成）、
> `.github/`（工作流占位 + issue/PR 模板）、`LICENSE`（MIT）、`CHANGELOG.md`、
> `CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`、`docs/DEVELOPMENT.md`、
> `docs/ROADMAP.md`，以及发布关键的两个契约文件：
>
> - `desktop-manifest.json` —— 组件版本锁（releasegraph 驱动桌面自动升级的输入，见 `docs/RELEASE-ECOSYSTEM-ANALYSIS.md`）
> - `desktop-config.json` —— 示例配置（运行时拷贝存于 OS app-config 目录）

---

## 4. 风险点

| # | 风险 | 缓解 |
|---|---|---|
| R1 | 目标机器缺 Node ≥ 22.13（MVP 受限于“node 启动”路径） | README 标注先决条件；Windows 预留 `command`（exe）覆盖路径；后续可打包 bun/pkg |
| R2 | 前端 `socket.ts` 生产环境为相对路径 ⇒ 前端必须与后端同源 | 已用同源托管方案从根上避免 |
| R3 | Tauri 主窗口加载外部 `http://127.0.0.1` 时无 Tauri JS bridge（`window.__TAURI__`） | MVP 业务（后端生命周期）全在 Rust；settings/托盘等后续功能用额外 `tauri://localhost` 窗口承载 bridge |
| R4 | Windows 上对子进程发送优雅信号受限（无 Ctrl+C 语义） | SIGTERM→等待→强杀兜底；文档标注 Windows 优雅停机为 best-effort，后续可用额外包装 |
| R5 | 打包体积/依赖（node_modules 整包） | 打包脚本可按需裁剪；MVP 允许较大体积，后续优化 |
| R6 | 构建环境需 Rust toolchain（本机当前缺失） | README 给出 `rustup` 安装命令；本机无法编译验证，代码遵循 Tauri 2 惯用 API |
| R7 | 后端首次启动会自举 DB 并可能弹出认证横幅 | 回环无认证即正常；横幅是后端固有 UI，不阻断 |
| R8 | `injectAuthBanner` 依赖 index.html 存在 | 正常被后端托管则存在；API-only 时后端有 `GET /` 信息响应，不影响桌面 |

---

## 5. 分阶段实施计划

- **P1 · MVP（本次）**：Tauri 窗口 + BackendManager（start/stop/restart/health）、config.json、同源 local 模式、remote 模式、进程/端口/重复启动处理、退出优雅关机、打包与 bundle 脚本、README。
- **P2 · 体验**：启动 splash 窗口 + 就绪后切换；托盘图标（最小化到托盘/退出）；后端异常的红点/状态提示。
- **P3 · 集成**：内置设置面板（改端口/数据/下载目录/模式，写入 config.json）；登录流程优化（复用 `window.electron` 钩子 → 改 Tauri bridge）；scheduler 状态、下载任务、日志查看面板。
- **P4 · 分发**：自动更新（tauri-updater）、Windows exe / macOS dmg / Linux deb-AppImage 签名与发布；Node 运行时内嵌化。

---

_（本文件为阶段 1 交付。上游两个仓库均未被修改。）_