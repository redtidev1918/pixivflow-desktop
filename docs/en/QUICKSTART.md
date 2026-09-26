# Quick Start

**Language / 语言:** [中文](/QUICKSTART.md) · English

From installer to up and running takes about five minutes. The only prerequisite: **your machine needs no preinstalled Node or runtime** — the desktop ships them inside the installer.

## 1. Download the installer

Get your platform's file from the [download page](/en/download.md) or [Releases](https://github.com/redtidev1918/pixivflow-desktop/releases):

| Platform | File |
| :-- | :-- |
| macOS (Apple Silicon) | `PixivFlow-Desktop-v<version>-macos-arm64.dmg` |
| Windows 10/11 (x64) | `PixivFlow-Desktop-v<version>-windows-x64-setup.exe` |
| Linux (x64, Debian/Ubuntu family) | `PixivFlow-Desktop-v<version>-linux-amd64.deb` |

To verify the download first, every release also carries `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
```

## 2. Install and allow the first launch

The installers are **not code-signed**, so the OS blocks them once:

- **macOS**: drag the app into Applications, then **right-click → Open** in Finder (or allow it under System Settings → Privacy & Security). The "unidentified developer" warning is expected.
- **Windows**: run `setup.exe` and pick *More info → Run anyway* on the SmartScreen prompt.
- **Linux**: `sudo apt install ./PixivFlow-Desktop-v<version>-linux-amd64.deb`, or double-click to install from your software centre.

## 3. Start it

Open **PixivFlow Desktop**. It automatically:

1. reads its config (creating `desktop-config.json` in the per-user app data directory on first run);
2. starts the bundled PixivFlow backend and waits for `/api/health` to pass;
3. opens the **WebUI** in the main window once healthy.

The window *is* the PixivFlow interface — not a wrapped browser, but the same WebUI served by the local backend over `STATIC_PATH` and hosted by a native window. While starting, the window footer shows backend status (port, PID, health); if the backend cannot start you get the manager panel instead of a white screen.

## 4. Sign in to Pixiv

Click sign-in in the WebUI: the Pixiv authorize page opens as an **overlay inside the app** — no external browser, no second window — and returns to PixivFlow when done. Give up mid-way with *Cancel sign-in* in the menu bar.

Credentials stay in your own data directory; the desktop never uploads or forwards them.

## 5. Where config and data live

| What | Where |
| :-- | :-- |
| Desktop config (port, auto-start, command override) | `desktop-config.json` in the per-user app data directory |
| PixivFlow config / data / downloads | `config/`, `data/`, `downloads/` in the per-user app data directory |
| Logs and the last-run record | the app log directory (*Open logs* in the menu bar) |

Upgrading or reinstalling the app therefore never loses your data.

## Common actions

| I want to… | How |
| :-- | :-- |
| Read the backend log | Menu bar → *Open logs* |
| Open the manager panel | Menu bar → *Open manager* |
| Report a problem | Menu bar → *Collect diagnostics*, then send the generated folder |
| Change the backend port | Edit the port in `desktop-config.json` and restart |
| Change the UI language | It follows the system language (Chinese / English); restart after changing it |

## Run from source

You need Node 20+, a Rust toolchain and the [Tauri prerequisites](https://tauri.app/start/prerequisites/):

```bash
npm install
npm run tauri dev        # development run
npm test                 # frontend unit tests + version consistency + dist check
cd src-tauri && cargo test   # backend lifecycle integration tests
```

The committed default backend is the light `dev-backend.mjs` stand-in, so a fresh clone runs immediately. To use the real runtime:

```bash
npm run runtime:fetch    # defaults to the sibling ../redtidev1918/PixivFlow
npm run webui:fetch      # compose the real WebUI into src-tauri/resources/webui/dist
```

Details: [development guide](/DEVELOPMENT.md) (in Chinese).

## If something goes wrong

Startup failures, busy ports, white screens and sign-in problems are covered in [troubleshooting](/en/TROUBLESHOOTING.md). Run *Collect diagnostics* before reporting — it saves a round trip.
