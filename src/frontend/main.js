// F1 frontend shell — status display only.
// All system capability is owned by Rust; this page only renders state
// and forwards user intent through Tauri commands (invoke).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ---- state rendering ----------------------------------------------------
const dot = document.getElementById("status-dot");
const label = document.getElementById("status-label");
const detail = document.getElementById("status-detail");
const metaPort = document.getElementById("meta-port");
const metaPid = document.getElementById("meta-pid");
const metaMode = document.getElementById("meta-mode");
const metaConfig = document.getElementById("meta-config");
const logEl = document.getElementById("log");
const modeEl = document.getElementById("mode");

const STATE_STYLES = {
  running: { dot: "dot-running", text: "Backend running", ok: true },
  starting: { dot: "dot-starting", text: "Starting…", ok: false },
  stopped: { dot: "dot-stopped", text: "Backend stopped", ok: true },
  error: { dot: "dot-error", text: "Error", ok: false },
  unknown: { dot: "dot-unknown", text: "未知状态", ok: false },
};

function render(status) {
  const s = STATE_STYLES[status.state] ?? STATE_STYLES.unknown;
  dot.className = "dot " + s.dot;
  label.textContent = s.text;
  label.style.color = s.ok ? "var(--fg)" : "var(--accent)";
  detail.textContent = status.message ?? "";
  metaPort.textContent = status.port ?? "—";
  metaPid.textContent = status.pid ?? (status.state === "starting" ? "启动中…" : "—");
  metaMode.textContent = status.mode;
  modeEl.textContent = status.mode;
  if (status.port) metaPort.textContent = status.port;
}

function appendLog(line) {
  const stamp = new Date().toLocaleTimeString();
  logEl.textContent += `[${stamp}] ${line}\n`;
  logEl.scrollTop = logEl.scrollHeight;
}

async function refresh() {
  try {
    render(await invoke("get_status"));
  } catch (e) {
    render({ state: "error", message: String(e) });
  }
}

async function withLock(fn) {
  try {
    render({ state: "starting", message: "执行中…" });
    render(await fn());
  } catch (e) {
    render({ state: "error", message: String(e) });
    appendLog("操作失败: " + e);
  }
}

// ---- bootstrap ----------------------------------------------------------
window.addEventListener("DOMContentLoaded", async () => {
  // keep status in sync via Rust events + a light poll
  listen("backend-status", (e) => render(e.payload));
  setInterval(refresh, 1000);

  document.getElementById("btn-start").addEventListener("click", (e) => {
    e.currentTarget.disabled = true;
    withLock(() => invoke("start_backend"));
    setTimeout(() => (e.currentTarget.disabled = false), 1500);
  });
  document.getElementById("btn-stop").addEventListener("click", (e) => {
    e.currentTarget.disabled = true;
    withLock(() => invoke("stop_backend"));
    setTimeout(() => (e.currentTarget.disabled = false), 1500);
  });
  document.getElementById("btn-restart").addEventListener("click", (e) => {
    e.currentTarget.disabled = true;
    withLock(() => invoke("restart_backend"));
    setTimeout(() => (e.currentTarget.disabled = false), 1500);
  });

  // load config path for display
  try {
    const cfg = await invoke("get_config");
    metaMode.textContent = cfg.mode ?? "local";
  } catch (_) {
    /* non-fatal */
  }
  try {
    metaConfig.textContent = await invoke("config_path");
  } catch (_) {
    metaConfig.textContent = "—";
  }

  await refresh();
  appendLog("前端壳已加载");
});