const BUNDLED_VERSION = '0.0.0-dev';
if (process.argv.includes('--version')) { console.log(`pixivflow ${BUNDLED_VERSION}`); process.exit(0); }

// F2.2 bundled runtime DEV STAND-IN.
// Proves the runtime/contract the desktop relies on:
//   - GET /api/health -> 200  (BackendManager.health_check contract)
//   - STATIC_PATH -> serves the bundled WebUI at /   (F2.2.3 方案 A)
//   - SIGTERM -> graceful close (BackendManager.stop contract)
// It carries NO PixivFlow business logic.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { join, normalize, resolve, extname } from "node:path";

const port = Number(process.env.PORT || 3000);
const host = process.env.HOST || "127.0.0.1";
const staticPath = process.env.STATIC_PATH || "";

const MIME = {
  ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript",
  ".css": "text/css", ".json": "application/json", ".png": "image/png",
  ".svg": "image/svg+xml", ".ico": "image/x-icon",
};

const server = createServer(async (req, res) => {
  const url = (req.url || "/").split("?")[0];
  if (req.method === "GET" && (url === "/api/health" || url === "/health")) {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ ok: true, service: "pixivflow-bundled-dev", status: "up" }));
    return;
  }
  if (req.method === "GET" && url === "/") {
    if (staticPath) {
      try {
        const html = await readFile(join(staticPath, "index.html"));
        res.writeHead(200, { "Content-Type": "text/html" });
        res.end(html);
        return;
      } catch (_) { /* fall through to built-in placeholder */ }
    }
    res.writeHead(200, { "Content-Type": "text/html" });
    res.end("<h1>Bundled runtime (F2.2 dev stand-in)</h1><p>STATIC_PATH-driven WebUI server.</p>");
    return;
  }
  if (req.method === "GET" && staticPath) {
    const safe = resolve(staticPath, "." + url.replace(/^(\/)+/, "/"));
    if (safe.startsWith(resolve(staticPath))) {
      try {
        const s = await stat(safe);
        if (s.isFile()) {
          const data = await readFile(safe);
          res.writeHead(200, { "Content-Type": MIME[extname(safe)] || "application/octet-stream" });
          res.end(data);
          return;
        }
      } catch (_) { /* 404 below */ }
    }
  }
  res.writeHead(404, { "Content-Type": "application/json" });
  res.end(JSON.stringify({ error: "not_found" }));
});

server.listen(port, host, () => console.log(`[bundled-dev] listening http://${host}:${port}`));
for (const sig of ["SIGTERM", "SIGINT"]) {
  process.on(sig, () => { console.log(`[bundled-dev] ${sig} -> closing`); server.close(() => process.exit(0)); });
}
