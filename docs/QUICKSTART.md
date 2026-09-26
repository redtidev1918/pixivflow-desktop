# 快速开始

**语言 / Language:** 中文 · [English](/en/QUICKSTART.md)

从安装包到跑起来大约五分钟。前置条件只有一个：**你的机器上不需要预先安装 Node 或任何运行时**——桌面版把它们打进了安装包。

## 1. 下载安装包

到[下载页](/download.md)或 [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) 取对应平台的文件：

| 平台 | 文件 |
| :-- | :-- |
| macOS（Apple Silicon） | `PixivFlow-Desktop-v<版本>-macos-arm64.dmg` |
| Windows 10/11（x64） | `PixivFlow-Desktop-v<版本>-windows-x64-setup.exe` |
| Linux（x64，Debian/Ubuntu 系） | `PixivFlow-Desktop-v<版本>-linux-amd64.deb` |

想先校验完整性的话，同一个 Release 里还带 `SHA256SUMS`：

```bash
sha256sum -c SHA256SUMS
```

## 2. 安装并首次打开

安装包**未做代码签名**，所以系统会拦第一次：

- **macOS**：把应用拖进「应用程序」，然后在访达里**右键 → 打开**（或「系统设置 → 隐私与安全性 → 仍要打开」）。双击打开时出现的「无法验证开发者」属于预期行为。
- **Windows**：运行 `setup.exe`，SmartScreen 提示里点「更多信息 → 仍要运行」。
- **Linux**：`sudo apt install ./PixivFlow-Desktop-v<版本>-linux-amd64.deb`，或双击用软件中心安装。

## 3. 启动

打开 **PixivFlow Desktop**。它会自动：

1. 读取配置（第一次运行会在每用户应用数据目录生成 `desktop-config.json`）；
2. 启动自带的 PixivFlow 后端，并等待 `/api/health` 通过；
3. 健康检查通过后，在主窗口打开 **WebUI**。

窗口就是 PixivFlow 的界面——它不是套壳浏览器，而是由本地后端用 `STATIC_PATH` 提供、由原生窗口承载的同一个 WebUI。启动过程中窗口底部会显示后端状态（端口、PID、健康状态）；如果后端起不来，会弹出管理面板而不是白屏。

## 4. 登录 Pixiv

在 WebUI 里点登录，Pixiv 授权页会以**浮层**形式在应用内打开：不跳外部浏览器、不开第二个窗口，授权完成后自动回到 PixivFlow。中途想放弃，用菜单栏的 *Cancel sign-in*。

登录凭据保存在你自己的数据目录里，桌面端不上传、不转发。

## 5. 配置与数据放在哪

| 内容 | 位置 |
| :-- | :-- |
| 桌面端配置（端口、自动启动、命令覆盖） | 每用户应用数据目录下的 `desktop-config.json` |
| PixivFlow 配置 / 数据 / 下载 | 每用户应用数据目录下的 `config/`、`data/`、`downloads/` |
| 日志与上次运行记录 | 应用日志目录（菜单栏 *Open logs* 直接打开） |

所以升级或重装应用不会丢数据。

## 常用动作

| 我想…… | 怎么做 |
| :-- | :-- |
| 看后端日志 | 菜单栏 → *Open logs* |
| 打开管理面板 | 菜单栏 → *Open manager* |
| 报告问题 | 菜单栏 → *Collect diagnostics*，把生成的目录打包发出来 |
| 改后端端口 | 编辑 `desktop-config.json` 里的端口后重启应用 |
| 让界面换语言 | 跟随系统语言（中文 / English），改系统语言后重启应用 |

## 从源码运行

需要 Node 20+、Rust 工具链和 [Tauri 前置依赖](https://tauri.app/start/prerequisites/)：

```bash
npm install
npm run tauri dev        # 开发运行
npm test                 # 前端单测 + 版本一致性 + dist 检查
cd src-tauri && cargo test   # 后端生命周期集成测试
```

仓库提交的默认后端是轻量 `dev-backend.mjs` 替身，全新 clone 即可启动。要用真实运行时：

```bash
npm run runtime:fetch    # 默认读兄弟目录 ../redtidev1918/PixivFlow
npm run webui:fetch      # 合成真实 WebUI 到 src-tauri/resources/webui/dist
```

细节见[开发指南](/DEVELOPMENT.md)。

## 遇到问题

启动失败、端口被占用、白屏、登录异常都写在[故障排查](/TROUBLESHOOTING.md)里；发反馈前先跑一次 *Collect diagnostics*，能省掉一轮来回。
