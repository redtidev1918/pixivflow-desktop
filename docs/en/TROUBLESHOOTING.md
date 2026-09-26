# Troubleshooting

**Language / 语言:** [中文](/TROUBLESHOOTING.md) · English

The desktop's rule is **no white screens, no silent failures**: if the backend cannot start you get the manager panel, and problems leave a log behind. Work in this order — collect evidence first, then match the symptom.

## First: collect diagnostics

Menu bar → **Collect diagnostics**. It bundles the following into `<log dir>/diagnostics-<UTC timestamp>/` and reveals it in your file manager:

| Item | What it is |
| :-- | :-- |
| Main log + rotations | Startup, runtime discovery, backend stdout/stderr, errors forwarded from the frontend |
| `last-run.json` | Whether the previous run exited cleanly or crashed |
| `crash-*.ips` | macOS native crash report (copied out of the system directory when the previous run was unclean, because the system purges the originals) |
| `doctor.json` | Backend self-check output |
| `config.json` | Your configuration with **secret-looking values replaced by `"***"`** |
| `env.txt` | Platform and version information |

To read the log yourself: menu bar → **Open logs**.

## The app will not start, or the window stays empty

The window footer shows backend status (port, PID, health). Match it:

| Symptom | Common cause | Fix |
| :-- | :-- | :-- |
| Manager panel says no runtime found | Running from source without fetching the real runtime | `npm run runtime:fetch` (packaged builds never hit this) |
| Status sits at "starting" and then fails | Another program holds the port | See the next section |
| Backend process exits immediately | Bundled `node` missing, blocked by antivirus, permissions | Read the backend's stderr in the log; send the diagnostics bundle |
| Blank window with no status line | The main window got no content | Look for frontend errors in the log (they are forwarded to the Rust side) |

Runtime discovery order is **bundle → config → PATH → mock**, and the log records which one won (`bundled` / `config` / `path` / `mock`). In development, `mock` is the light stand-in committed to the repo — not the real PixivFlow.

## The port is taken

The default port is **3000** (`backend.port` in `desktop-config.json`).

- If the process holding the port **is a leftover PixivFlow backend from a crash**, the desktop **adopts** it instead of starting a second one — the status shows the adopted process. Adoption only happens when the listener's command line mentions `pixivflow`, so an unrelated service is never stolen.
- If **another program** holds the port, the desktop will not kill it. Either move that program or change the port:

```jsonc
// <app data dir>/desktop-config.json
{
  "mode": "local",
  "backend": { "port": 3100, "autoStart": true }
}
```

Restart the app afterwards. `backend.command` / `backend.args` can override how the backend is launched (by default it uses the bundled runtime).

## White screen or missing styles

The desktop does not render the frontend itself: it starts the local backend, the backend serves the WebUI over `STATIC_PATH`, and the window loads that address. So:

- **Backend not running** → you should see the manager panel, not a white screen. If it really is blank, collect diagnostics and look for a backend exit record.
- **Backend running but the page is empty** → the WebUI assets are most likely absent. Packaged builds include them; **running from source** needs `npm run webui:fetch`, otherwise the placeholder page is used.
- Frontend exceptions are forwarded to the Rust log through the host bridge (`log_frontend`), so the diagnostics bundle contains them.

## Sign-in issues

- **The authorize page never appears**: dismiss with *Cancel sign-in* in the menu bar and retry. It opens as an in-app **overlay**, never an external browser.
- **Authorization completes but does not return to PixivFlow**: make sure you did not close the overlay; retry the sign-in. If it keeps failing, run the upstream sign-in flow from the CLI once to see whether it is an account/network problem or a desktop problem.
- **You need a proxy**: the desktop does not manage network settings — configure the proxy in PixivFlow's own config.
- Credentials stay in your data directory; the desktop never forwards or uploads them.

## Will I lose my data?

No. Your PixivFlow data lives in the **per-user app data directory** (`config/`, `data/`, `downloads/`), not in the application directory, so upgrading, reinstalling or switching installers never touches it. The desktop's own configuration is `desktop-config.json` in the same layer. To back everything up, copy that directory.

## What to include in a report

1. The diagnostics directory (zipped) — it already contains the logs, `last-run.json`, crash reports, `doctor.json` and the redacted config;
2. Your OS version and the desktop version (menu/about, or the `PixivFlow-Desktop-v<version>-*` filename);
3. Reproduction steps: what you clicked, what you expected, what happened.

Those three usually save a round trip. If it is still unsolved, file an issue with the bundle at [Issues](https://github.com/redtidev1918/pixivflow-desktop/issues).
