// Writes one version into every file that carries it.
//
// release-please bumps these through `extra-files` (see
// release-please-config.json), so this script is the fallback for the two cases
// that config cannot cover: a hand-cut release rehearsal, and a build that
// discovers the committed version drifted from the tag it is shipping under
// (scripts/build-release calls it in exactly that situation, because shipping an
// installer whose bundle version disagrees with its release is worse than
// rewriting the three files during the build).
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error(`usage: node scripts/set-release-version.mjs <semver>  (got: ${version ?? "<missing>"})`);
  process.exit(2);
}

const root = process.cwd();
const writes = [];

const pkgPath = join(root, "package.json");
const pkg = readFileSync(pkgPath, "utf8");
if (!pkg.includes(`"version": "${version}"`)) {
  writeFileSync(pkgPath, pkg.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`));
  writes.push("package.json");
}

const tauriPath = join(root, "src-tauri/tauri.conf.json");
const tauri = readFileSync(tauriPath, "utf8");
if (!tauri.includes(`"version": "${version}"`)) {
  writeFileSync(tauriPath, tauri.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`));
  writes.push("src-tauri/tauri.conf.json");
}

const cargoPath = join(root, "src-tauri/Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
if (!new RegExp(`^version\\s*=\\s*"${version.replace(/\./g, "\\.")}"`, "m").test(cargo)) {
  // Only the first `version = "..."` line is the package version: dependency
  // tables may repeat the key, so the package line must win.
  writeFileSync(cargoPath, cargo.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`));
  writes.push("src-tauri/Cargo.toml");
}

if (writes.length === 0) {
  console.log(`all version files already say ${version}`);
} else {
  console.log(`set ${version} in: ${writes.join(", ")}`);
}
