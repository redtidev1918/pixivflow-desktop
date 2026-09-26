// Bootstrap UI controller. Responsibilities limited to: render status, react
// to button clicks, surface errors. All Tauri calls are try/catch-wrapped so
// an IPC failure renders an error state instead of a blank page.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { deriveView } from "./state.js";

const el = {
  badge: document.getElementById("badge"),
  line: document.getElementById("status-line"),
  port: document.getElementById("meta-port"),
  pid: document.getElementById("meta-pid"),
  health: document.getElementById("meta-health"),
  errorBox: document.getElementById("error-box"),
  errorTitle: document.getElementById("error-title"),
  errorLabel: document.getElementById("error-reason"),
  errorDetail: document.getElementById("error-detail"),
  errorRestart: document.getElementById("btn-error-restart"),
  openWebui: document.getElementById("btn-open-webui"),
  restart: document.getElementById("btn-restart"),
  stop: document.getElementById("btn-stop"),
  openLogs: document.getElementById("btn-open-logs"),
  footSrc: document.getElementById("foot-src"),
  footVer: document.getElementById("foot-ver"),
  footMode: document.getElementById("foot-mode"),
};

function renderView(status) {
  const v = deriveView(status);
  el.badge.className = "badge " + v.dot;
  el.badge.textContent = v.badgeText;
  el.line.textContent = v.line;
  el.port.textContent = v.port;
  el.pid.textContent = v.pid;
  el.health.textContent = v.health;
  if (v.showError) {
    el.errorTitle.textContent = v.errorTitle;
    el.errorLabel.textContent = v.errorLabel;
    el.errorDetail.textContent = v.message || "";
    el.errorBox.classList.remove("hidden");
  } else {
    el.errorBox.classList.add("hidden");
  }
}

// Never let any single failure blank the page — route it to the error surface.
function renderError(error) {
  const msg = error && error.message ? String(error.message) : String(error);
  renderView({ state: "error", message: msg });
}

async function refresh() {
  try {
    const status = await invoke("get_status");
    renderView(status);
  } catch (err) {
    renderError(err);
  }
}

async function loadDoctor() {
  try {
    const d = await invoke("backend_doctor");
    el.footSrc.textContent = "source: " + (d && d.source != null ? d.source : "?");
    el.footVer.textContent = "version: " + (d && d.version != null ? d.version : "?");
  } catch (_) {
    /* non-fatal footer info */
  }
}

async function restart(action) {
  try {
    const status = await invoke(action);
    renderView(status);
  } catch (err) {
    renderError(err);
  }
}

function bindButton(btn, action) {
  btn.addEventListener("click", async () => {
    btn.disabled = true;
    await restart(action);
    setTimeout(() => (btn.disabled = false), 1200);
  });
}

window.addEventListener("DOMContentLoaded", () => {
  // Immediate first frame from boot shell (already visible); then sticky events.
  const shell = { state: "starting" };
  renderView(shell);
  renderView(shell); // idempotent

  try {
    listen("backend-status", (e) => renderView(e && e.payload ? e.payload : shell));
  } catch (err) {
    renderError(err);
  }

  bindButton(el.restart, "restart_backend");
  bindButton(el.errorRestart, "restart_backend");
  bindButton(el.stop, "stop_backend");
  el.openWebui.addEventListener("click", async () => {
    try {
      const url = await invoke("open_webui");
      el.line.textContent = "Open WebUI: " + url;
    } catch (err) {
      renderError(err);
    }
  });
  el.openLogs.addEventListener("click", async () => {
    try {
      await invoke("open_logs");
    } catch (err) {
      renderError(err);
    }
  });

  loadDoctor();
  refresh();
  setInterval(refresh, 1000);
});
