// F1 mock backend — a health-probe stub ONLY (timing/contract testing).
// It is NOT PixivFlow and carries no business logic. Real backend arrives in F2.
import { createServer } from "node:http";

const port = Number(process.env.PORT || 3000);
const host = process.env.HOST || "127.0.0.1";

const server = createServer((req, res) => {
  if (req.method === "GET" && (req.url === "/api/health" || req.url === "/health")) {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ ok: true, service: "pixivflow-mock", status: "up" }));
    return;
  }
  if (req.method === "GET" && req.url === "/") {
    res.writeHead(200, { "Content-Type": "text/html" });
    res.end("<h1>Mock backend (F1)</h1><p>placeholder;\u00a0real WebUI arrives in F2.</p>");
    return;
  }
  res.writeHead(404, { "Content-Type": "application/json" });
  res.end(JSON.stringify({ error: "not_found" }));
});

server.listen(port, host, () => {
  console.log(`[mock-backend] listening on http://${host}:${port}`);
});

// graceful shutdown on SIGTERM so BackendManager.stop() sees a clean exit.
for (const sig of ["SIGTERM", "SIGINT"]) {
  process.on(sig, () => {
    console.log(`[mock-backend] ${sig} -> closing`);
    server.close(() => process.exit(0));
  });
}
