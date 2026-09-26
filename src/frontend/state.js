// Pure, DOM-free view-state derivation. Mapping a status payload to what the
// UI should show. No Tauri imports — unit-testable in plain Node.
//
// Guarantee: this NEVER throws for any input, so a bad/empty event payload can
// never blank the page (it falls back to a safe "unknown" state).

export const STATES = {
  starting: { text: "Starting", dot: "badge-starting" },
  running:  { text: "Running",  dot: "badge-running" },
  stopped:  { text: "Stopped",  dot: "badge-stopped" },
  error:    { text: "Error",    dot: "badge-error" },
  unknown:  { text: "Unknown",  dot: "badge-unknown" },
};

const NOT_FOUND_RE = /runtime\s+(not\s+found|missing|unresolved)|未找到\s*(runtime|后端)|runtime\s*不存在|runtime.*missing/i;

function classify(msg) {
  // 1) an explicit failure message (backend start failure / missing runtime)
  const m = msg ? String(msg) : "";
  if (m && NOT_FOUND_RE.test(m)) return "not-found";
  if (m) return "failed";
  return "error";
}

export function deriveView(status) {
  const raw = status && typeof status === "object" ? status : null;
  const key = raw && STATES[raw.state] ? raw.state : "unknown";
  const st = STATES[key];

  const port = raw && raw.port != null ? String(raw.port) : "—";
  const pid = raw && raw.pid != null ? String(raw.pid) : key === "starting" ? "启动中…" : "—";

  let health = "—";
  if (key === "running") health = raw.healthy ? "Healthy" : "Unhealthy";
  else if (key === "starting") health = "Waiting…";

  let line = "Starting PixivFlow…";
  if (key === "running") line = raw.healthy ? "PixivFlow running" : "Waiting for backend…";
  else if (key === "stopped") line = "Backend stopped";
  else if (key === "error" || key === "unknown") line = "Backend failed";

  const message = raw && raw.message ? String(raw.message) : "";
  const kind = classify(raw && raw.message != null ? raw.message : "");

  // error UX variants (no blank window):
  //  - runtime not found -> "PixivFlow runtime not found"
  //  - start failure     -> "Backend failed · Reason: …"
  //  - health stall      -> the `starting` branch shows "Waiting for backend…"
  let errorTitle = "";
  let errorLabel = "";
  let showError = false;
  if (key === "error" || key === "unknown" || kind !== "error") {
    showError = true;
    if (kind === "not-found") {
      errorTitle = "PixivFlow runtime not found";
      errorLabel = "No PixivFlow runtime resolved.";
    } else if (kind === "failed" || key === "unknown") {
      errorTitle = "Backend failed";
      errorLabel = "Reason:";
    } else {
      errorTitle = "Backend Error";
      errorLabel = "Reason:";
    }
  }

  return {
    keyword: key,
    badgeText: st.text,
    dot: st.dot,
    port,
    pid,
    health,
    line,
    message,
    showError,
    errorTitle,
    errorLabel,
  };
}
