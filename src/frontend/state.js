// Pure, DOM-free view-state derivation. Mapping a status payload to what the
// UI should show. No Tauri imports — unit-testable in plain Node.
//
// Guarantee: this NEVER throws for any input, so a bad/empty event payload can
// never blank the page (it falls back to a safe "unknown" state).
//
// Every user-visible string comes from the translator `t` (see i18n.js), so the
// launcher speaks the system language; the default is English, which is also
// what the unit tests pin.

import { FALLBACK_LOCALE, makeT } from "./i18n.js";

export const STATES = {
  starting: { text: "Starting", i18nKey: "badge.starting", dot: "badge-starting" },
  running:  { text: "Running",  i18nKey: "badge.running",  dot: "badge-running" },
  stopped:  { text: "Stopped",  i18nKey: "badge.stopped",  dot: "badge-stopped" },
  error:    { text: "Error",    i18nKey: "badge.error",    dot: "badge-error" },
  unknown:  { text: "Unknown",  i18nKey: "badge.unknown",  dot: "badge-unknown" },
};

const NOT_FOUND_RE = /runtime\s+(not\s+found|missing|unresolved)|未找到\s*(runtime|后端)|runtime\s*不存在|runtime.*missing/i;

function classify(msg) {
  // 1) an explicit failure message (backend start failure / missing runtime)
  const m = msg ? String(msg) : "";
  if (m && NOT_FOUND_RE.test(m)) return "not-found";
  if (m) return "failed";
  return "error";
}

export function deriveView(status, t = makeT(FALLBACK_LOCALE)) {
  const raw = status && typeof status === "object" ? status : null;
  const key = raw && STATES[raw.state] ? raw.state : "unknown";
  const st = STATES[key];

  const port = raw && raw.port != null ? String(raw.port) : "—";
  const pid = raw && raw.pid != null ? String(raw.pid) : key === "starting" ? t("meta.starting") : "—";

  let health = "—";
  if (key === "running") health = raw.healthy ? t("health.healthy") : t("health.unhealthy");
  else if (key === "starting") health = t("meta.waiting");

  let line = t("line.starting");
  if (key === "running") line = raw.healthy ? t("line.running") : t("line.waitingBackend");
  else if (key === "stopped") line = t("line.stopped");
  else if (key === "error" || key === "unknown") line = t("line.failed");

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
      errorTitle = t("error.runtimeNotFound");
      errorLabel = t("error.noRuntime");
    } else if (kind === "failed" || key === "unknown") {
      errorTitle = t("error.backendFailed");
      errorLabel = t("error.reason");
    } else {
      errorTitle = t("error.backendError");
      errorLabel = t("error.reason");
    }
  }

  return {
    keyword: key,
    badgeText: t(st.i18nKey),
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
