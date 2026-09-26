// Build smoke test: the bundle must ship a REAL PixivFlow runtime.
//
// `src-tauri/resources/runtime/pixivflow/` keeps the dev stand-in (`VERSION`
// `0.0.0-dev`, manifest command `["node","./dev-backend.mjs"]`) in the
// repository, because the real payload is ~200 MB and never committed. A build
// therefore copies whatever is on disk, and a clean checkout silently produces
// an app whose backend answers `/api/health` while downloading nothing — the
// failure mode this check exists to catch. The dev stand-in is still allowed,
// but only when asked for out loud.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const runtimeDir = join(process.cwd(), "src-tauri", "resources", "runtime", "pixivflow");
const manifestPath = join(runtimeDir, "runtime-manifest.json");
const versionPath = join(runtimeDir, "VERSION");
const devStandInAllowed = process.env.PIXIVFLOW_ALLOW_DEV_RUNTIME === "1";

const fail = (msg) => {
  console.error("✗ " + msg);
  process.exit(1);
};

const ok = (msg) => console.log("✓ " + msg);

if (!existsSync(manifestPath)) fail("runtime-manifest.json is missing from the bundle resources");

let manifest;
try {
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (error) {
  fail(`runtime-manifest.json is not valid JSON: ${error.message}`);
}

const version = existsSync(versionPath) ? readFileSync(versionPath, "utf8").trim() : "";
const command = Array.isArray(manifest.command) ? manifest.command : [];
const args = Array.isArray(manifest.args) ? manifest.args : [];
const entry = args.find((arg) => typeof arg === "string" && arg.endsWith(".js"));
const isStandIn =
  version.startsWith("0.0.0-dev") ||
  command.some((part) => String(part).includes("dev-backend")) ||
  entry === undefined ||
  entry.includes("dev-backend");

if (isStandIn) {
  if (!devStandInAllowed) {
    console.error(
      "✗ the bundle would ship the dev stand-in runtime " +
        `(version=${version || "unknown"}, entry=${entry ?? "none"}).\n` +
        "  Lay the real runtime first:  node scripts/fetch-pixivflow-runtime.mjs\n" +
        "  A dev stand-in build is only allowed with PIXIVFLOW_ALLOW_DEV_RUNTIME=1."
    );
    process.exit(1);
  }
  console.warn(
    "! shipping the DEV stand-in runtime — this build is not releasable " +
      `(version=${version || "unknown"})`
  );
  process.exit(0);
}

// The manifest is a promise: the executable and the entry it names must be in
// the payload, not merely described by it.
const executable = command[0];
if (typeof executable !== "string") fail("runtime-manifest.json has no command to run");
if (executable.startsWith("./")) {
  const resolved = join(runtimeDir, executable.slice(2));
  if (!existsSync(resolved)) fail(`the bundled runtime executable is missing: ${executable}`);
}
if (!existsSync(join(runtimeDir, entry))) {
  fail(`the bundled runtime entry is missing: ${entry}`);
}

ok(`runtime payload is real: pixivflow@${version || "unknown"} (${entry})`);
