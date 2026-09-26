// Build smoke test: after `npm run build`, verifies dist ships a branded,
// relative-based index.html + assets — the exact fix for the white-screen bug.
import { readFileSync, existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const dist = join(process.cwd(), "dist");
const htmlPath = join(dist, "index.html");
const assetsDir = join(dist, "assets");

const ok = (cond, msg) => {
  if (!cond) {
    console.error("✗ " + msg);
    process.exitCode = 1;
  } else {
    console.log("✓ " + msg);
  }
};

ok(existsSync(htmlPath), "dist/index.html exists");
ok(existsSync(assetsDir) && readdirSync(assetsDir).length > 0, "dist/assets/ is non-empty");

const html = readFileSync(htmlPath, "utf8");
ok(html.includes("PixivFlow Desktop"), "index.html is branded 'PixivFlow Desktop'");
ok(!/src="\/assets\//.test(html) && !/href="\/assets\//.test(html),
  "index.html uses RELATIVE base (no /assets absolute paths — white-screen cause)");

if (process.exitCode) {
  console.error("\nBuild output failed verification.");
  process.exit(process.exitCode);
}
