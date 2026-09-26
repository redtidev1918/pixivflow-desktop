# PixivFlow Desktop

**Language / 语言:** [中文](/) · English

> **The native desktop build of [PixivFlow](https://github.com/redtidev1918/PixivFlow), bundling the [pixivflow-webui](https://github.com/redtidev1918/pixivflow-webui) interface, for Windows / macOS / Linux.**

PixivFlow Desktop packages the PixivFlow runtime together with its WebUI into a native app you just double-click: nothing else has to be installed, and no server has to be started. The desktop layer only launches, supervises and bundles the local runtime; **all business logic stays in upstream PixivFlow**.

The project front page is the repository [README](https://github.com/redtidev1918/pixivflow-desktop/blob/main/README.en.md); this site carries the detailed installation, troubleshooting, architecture and release documentation.

## Start with these three

| I want to… | Go to |
| :-- | :-- |
| Install and use it | [📥 Download](/en/download.md) → [Quick Start](/en/QUICKSTART.md) |
| Fix a startup failure, white screen or sign-in problem | [Troubleshooting](/en/TROUBLESHOOTING.md) |
| Change code or cut a release | [Development](https://github.com/redtidev1918/pixivflow-desktop/blob/main/docs/DEVELOPMENT.md) · [Release](https://github.com/redtidev1918/pixivflow-desktop/blob/main/docs/RELEASE.md) (in Chinese) |

Installers live on [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases) (versions and checksums on the [download page](/en/download.md)):

| Platform | File |
| :-- | :-- |
| macOS (Apple Silicon) | `PixivFlow-Desktop-v<version>-macos-arm64.dmg` |
| Windows 10/11 | `PixivFlow-Desktop-v<version>-windows-x64-setup.exe` |
| Linux (x64) | `PixivFlow-Desktop-v<version>-linux-amd64.deb` |

> The installers are **unsigned**, so the OS blocks them once: macOS → right-click → Open; Windows → SmartScreen → *Run anyway*.
> macOS currently produces an Apple Silicon build only — see [release management](https://github.com/redtidev1918/pixivflow-desktop/blob/main/docs/RELEASE.md) (in Chinese).

## What it does, and does not do

**Does**: package and launch the local PixivFlow backend (carrying its own `node`), serve the WebUI over `STATIC_PATH`, manage the backend's start / adopt / graceful stop / restart, open the Pixiv sign-in page inside the app window, follow the system language, and collect a secret-free diagnostics bundle in one click.

**Does not**: implement any Pixiv business logic, duplicate a second frontend, or auto-update (those belong to upstream, upstream, and the roadmap).

Read [architecture](https://github.com/redtidev1918/pixivflow-desktop/blob/main/docs/ARCHITECTURE.md) for where the boundaries sit and [relationship](https://github.com/redtidev1918/pixivflow-desktop/blob/main/docs/RELATIONSHIP.md) for where runtime versions come from (both in Chinese).

## How this site is maintained

- The text is the repository's own Markdown: Chinese under `docs/*.md`, English under `docs/en/*.md`. Push to `main` and Actions redeploys.
- The [download page](/en/download.md) is never hand-written: a workflow refreshes it from the release data after every publication.
- The sidebar is a hand-maintained navigation of **independent pages** only; in-page heading structure stays out of it.

## Contributing

See [CONTRIBUTING.md](https://github.com/redtidev1918/pixivflow-desktop/blob/main/CONTRIBUTING.md); security issues → [SECURITY.md](https://github.com/redtidev1918/pixivflow-desktop/blob/main/SECURITY.md); community guidelines → [CODE_OF_CONDUCT.md](https://github.com/redtidev1918/pixivflow-desktop/blob/main/CODE_OF_CONDUCT.md). License: MIT © [redtidev1918](https://github.com/redtidev1918).
