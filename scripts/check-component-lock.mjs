#!/usr/bin/env node
// Component-lock gate: the desktop build derives both halves of the bundled
// product from `desktop-manifest.json`, and each half comes from a real remote
// source that must already offer the pinned version:
//
//   * `components.pixivflow`        -> npm registry  `pixivflow@<ver>`
//   * `components['pixivflow-webui']` -> GitHub tag `v<ver>` on
//                                        redtidev1918/pixivflow-webui
//
// If the lock names a version the source has not published yet, the release
// would silently fall back to "whatever the fetch resolves to" (a stale sibling
// checkout, a missing clone branch, an older npm) instead of the pinned build —
// exactly the drift that shipped WebUI 1.1.0 inside a bundle whose lock said
// 2.0.0. This script turns that into a hard failure *before* the bundle exists.
//
//   node scripts/check-component-lock.mjs
//   (exits 0 when every pinned version is publishable, 1 when one is missing)
//
// Network is required: CI and the release build have it; it is deliberately NOT
// wired into `npm test`, which runs offline in a cold checkout.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const WEBUI_REPO = "https://github.com/redtidev1918/pixivflow-webui.git";
const MANIFEST = "desktop-manifest.json";

const root = process.cwd();
const fail = (msg) => { console.error("✗ " + msg); process.exitCode = 1; };
const ok = (msg) => console.log("✓ " + msg);

let manifest;
try {
  manifest = JSON.parse(readFileSync(join(root, MANIFEST), "utf8"));
} catch (e) {
  console.error(`✗ cannot read ${MANIFEST}: ${e.message}`);
  process.exit(2);
}

const components = manifest.components;
if (!components || typeof components !== "object") {
  console.error(`✗ ${MANIFEST} has no "components" object`);
  process.exit(2);
}

// -- npm registry check (pixivflow) ----------------------------------------
const runtimeVer = components.pixivflow?.version;
if (typeof runtimeVer !== "string" || !runtimeVer) {
  fail(`${MANIFEST}: components.pixivflow.version is missing`);
} else {
  try {
    const out = execFileSync("npm", ["view", `pixivflow@${runtimeVer}`, "version"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
    }).trim();
    ok(`pixivflow@${runtimeVer} is published on npm (resolves to ${out})`);
  } catch {
    fail(`pixivflow@${runtimeVer} is NOT on the npm registry — the lock leads the published artifact`);
  }
}

// -- GitHub tag check (pixivflow-webui) ------------------------------------
// fetch-pixivflow-webui.mjs clones `--branch v<ver>`, so the check mirrors the
// exact ref the build will fetch.
const webuiVer = components["pixivflow-webui"]?.version;
if (typeof webuiVer !== "string" || !webuiVer) {
  fail(`${MANIFEST}: components['pixivflow-webui'].version is missing`);
} else {
  const ref = `refs/tags/v${webuiVer}`;
  try {
    const out = execFileSync(
      "git",
      ["ls-remote", "--tags", "--exit-code", WEBUI_REPO, ref],
      { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
    ).trim();
    ok(`pixivflow-webui tag ${ref} exists on GitHub (${out.split(/\s+/)[0]})`);
  } catch {
    fail(`pixivflow-webui tag ${ref} does NOT exist on GitHub — the lock leads the published artifact`);
  }
}

if (process.exitCode) {
  console.error(`\n${MANIFEST} pins versions that are not yet available at their build source.`);
  console.error("Publish the upstream versions first, or lower the lock to what the source already offers.");
  process.exit(process.exitCode);
}
console.log("\n✓ component lock is fully publishable");