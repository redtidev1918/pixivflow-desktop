// Locale plumbing for the bootstrap shell.
//
// The launcher must speak the SYSTEM language, not a hard-coded mix: the Rust
// side resolves it (see src-tauri/src/i18n.rs, command `get_locale`) and
// main.js asks for it once on boot. Everything here is pure and
// dependency-free so it can be unit-tested in plain Node, exactly like
// state.js.
//
// Rules:
//  - a language we do not ship falls back to English (never to a mix of both);
//  - every key exists in every locale (i18n.test.mjs enforces it).

export const LOCALES = ["en", "zh"];

export const FALLBACK_LOCALE = "en";

// Normalize a BCP-47 tag or locale id — "zh-Hans-CN", "zh_CN", "en-US", "en" —
// to one of LOCALES, or null when we ship no bundle for that language.
export function normalizeLocale(raw) {
  if (raw == null) return null;
  const tag = String(raw).trim().toLowerCase().replace(/_/g, "-");
  if (!tag) return null;
  const primary = tag.split("-")[0];
  return LOCALES.includes(primary) ? primary : null;
}

export const MESSAGES = {
  en: {
    "app.tagline": "The desktop runtime and manager for PixivFlow",
    "card.backendStatus": "Backend Status",
    "meta.port": "Port",
    "meta.pid": "PID",
    "meta.health": "Health",
    "meta.waiting": "Waiting…",
    "meta.starting": "starting…",
    "badge.starting": "Starting",
    "badge.running": "Running",
    "badge.stopped": "Stopped",
    "badge.error": "Error",
    "badge.unknown": "Unknown",
    "health.healthy": "Healthy",
    "health.unhealthy": "Unhealthy",
    "line.starting": "Starting PixivFlow…",
    "line.running": "PixivFlow running",
    "line.waitingBackend": "Waiting for backend…",
    "line.stopped": "Backend stopped",
    "line.failed": "Backend failed",
    "line.openWebui": "PixivFlow WebUI opened: {url}",
    "error.backendError": "Backend Error",
    "error.backendFailed": "Backend failed",
    "error.runtimeNotFound": "PixivFlow runtime not found",
    "error.noRuntime": "No PixivFlow runtime resolved.",
    "error.reason": "Reason:",
    "action.openWebui": "Open PixivFlow",
    "action.restart": "Restart",
    "action.stop": "Stop",
    "action.openLogs": "Open Logs",
    "title.openWebui": "Open the PixivFlow WebUI served by the backend",
    "title.restart": "Restart the local backend",
    "title.stop": "Stop the local backend",
    "title.openLogs": "Open logs/desktop.log",
    "foot.source": "source: {value}",
    "foot.version": "version: {value}",
    "foot.mode": "mode: {value}",
  },
  zh: {
    "app.tagline": "PixivFlow 的桌面运行环境和管理器",
    "card.backendStatus": "后端状态",
    "meta.port": "端口",
    "meta.pid": "PID",
    "meta.health": "健康检查",
    "meta.waiting": "等待中…",
    "meta.starting": "启动中…",
    "badge.starting": "启动中",
    "badge.running": "运行中",
    "badge.stopped": "已停止",
    "badge.error": "错误",
    "badge.unknown": "未知",
    "health.healthy": "正常",
    "health.unhealthy": "异常",
    "line.starting": "正在启动 PixivFlow…",
    "line.running": "PixivFlow 运行中",
    "line.waitingBackend": "等待后端响应…",
    "line.stopped": "后端已停止",
    "line.failed": "后端启动失败",
    "line.openWebui": "已打开 PixivFlow WebUI：{url}",
    "error.backendError": "后端错误",
    "error.backendFailed": "后端启动失败",
    "error.runtimeNotFound": "未找到 PixivFlow 运行时",
    "error.noRuntime": "没有找到可用的 PixivFlow 运行时。",
    "error.reason": "原因：",
    "action.openWebui": "打开 PixivFlow",
    "action.restart": "重启",
    "action.stop": "停止",
    "action.openLogs": "打开日志",
    "title.openWebui": "打开由后端提供的 PixivFlow WebUI",
    "title.restart": "重启本地后端",
    "title.stop": "停止本地后端",
    "title.openLogs": "打开 logs/desktop.log",
    "foot.source": "来源：{value}",
    "foot.version": "版本：{value}",
    "foot.mode": "模式：{value}",
  },
};

function interpolate(text, params) {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, key) =>
    params[key] != null ? String(params[key]) : match,
  );
}

// makeT(locale) -> t(key, params?) with English as the per-key fallback, so a
// missing translation degrades to English instead of leaking a raw key.
export function makeT(locale, messages = MESSAGES) {
  const dict = messages[locale] || messages[FALLBACK_LOCALE] || {};
  const fallback = messages[FALLBACK_LOCALE] || {};
  return (key, params) => interpolate(dict[key] ?? fallback[key] ?? key, params);
}

// The BCP-47 tag to put on <html lang>.
export function htmlLang(locale) {
  return locale === "zh" ? "zh-CN" : "en";
}
