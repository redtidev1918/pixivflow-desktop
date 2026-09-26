# PixivFlow Desktop

**语言 / Language:** 中文 · [English](/en/)

> **基于 [PixivFlow](https://github.com/redtidev1918/PixivFlow) 的原生桌面版，支持 Windows / macOS / Linux。**

PixivFlow Desktop 把 PixivFlow 运行时和它的 WebUI 一起打包成一个**双击即用**的原生应用：装完不用另外装 Node，也不用自己起服务。桌面端只负责启动、守护与打包本地运行时，**业务逻辑全部留在上游 PixivFlow**。

项目门面见仓库的 [README](https://github.com/redtidev1918/pixivflow-desktop/blob/main/README.md)；这个站点承载安装、排障、架构与发布的详细文档。

## 先做这三件事

| 我想…… | 去哪 |
| :-- | :-- |
| 装上用起来 | [📥 下载](/download.md) → [快速开始](/QUICKSTART.md) |
| 起不来 / 白屏 / 登录异常 | [故障排查](/TROUBLESHOOTING.md) |
| 改代码或发版 | [开发指南](/DEVELOPMENT.md) · [发布与版本管理](/RELEASE.md) |

安装包从 [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) 取（版本与校验和见[下载页](/download.md)）：

| 平台 | 文件 |
| :-- | :-- |
| macOS（Apple Silicon） | `PixivFlow-Desktop-v<版本>-macos-arm64.dmg` |
| Windows 10/11 | `PixivFlow-Desktop-v<版本>-windows-x64-setup.exe` |
| Linux（x64） | `PixivFlow-Desktop-v<版本>-linux-amd64.deb` |

> 安装包**未签名**，首次打开需要手动放行（macOS：右键 → 打开；Windows：SmartScreen → 仍要运行）。
> 目前 macOS 只出 Apple Silicon 版，原因见[发布与版本管理](/RELEASE.md)。

## 它做什么，不做什么

**做**：打包并启动本地 PixivFlow 后端（自带 `node`）、用 `STATIC_PATH` 提供 WebUI、管理后端的启动 / 接管 / 优雅停止 / 重启、把 Pixiv 登录页嵌进应用窗口、跟随系统语言的界面、一键收集不含密钥的诊断包。

**不做**：不实现任何 Pixiv 业务逻辑、不复制第二个前端、不做自动更新（分别属于上游、上游与路线图）。

边界怎么划的看[架构说明](/ARCHITECTURE.md)；运行时版本从哪来、和谁锁在一起看[生态关系](/RELATIONSHIP.md)。

## 这个站点怎么维护

- 正文就是仓库里的 Markdown：中文在 `docs/*.md`，英文在 `docs/en/*.md`；改完推 `main`，Actions 自动重新部署。
- [下载页](/download.md)不用手写：每次发版后由 workflow 依据 Release 数据自动刷新。
- 侧边栏是人工维护的导航，只收录**独立页面**，页面内的标题结构不往侧边栏塞。

## 参与

贡献见 [CONTRIBUTING.md](https://github.com/redtidev1918/pixivflow-desktop/blob/main/CONTRIBUTING.md)；
安全问题请走 [SECURITY.md](https://github.com/redtidev1918/pixivflow-desktop/blob/main/SECURITY.md)；
社区准则见 [CODE_OF_CONDUCT.md](https://github.com/redtidev1918/pixivflow-desktop/blob/main/CODE_OF_CONDUCT.md)。
许可证：MIT © [redtidev1918](https://github.com/redtidev1918)。
