// Release metadata gate: the three files that carry the desktop version must
// always agree. release-please bumps them together (see `extra-files` in
// release-please-config.json), and the release build refuses to ship a bundle
// whose own version disagrees with the tag it is published under — this script
// is what turns a forgotten bump into a red check *before* the tag exists.
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const ok = (cond, msg) => {
  if (!cond) {
    console.error("✗ " + msg);
    process.exitCode = 1;
  } else {
    console.log("✓ " + msg);
  }
};

const pkgVersion = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const tauriVersion = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8")).version;

const cargo = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
const cargoVersion = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];

ok(typeof cargoVersion === "string", "src-tauri/Cargo.toml declares a package version");
ok(
  pkgVersion === tauriVersion && pkgVersion === cargoVersion,
  `package.json (${pkgVersion}), src-tauri/tauri.conf.json (${tauriVersion}) and src-tauri/Cargo.toml (${cargoVersion}) agree`,
);

if (process.exitCode) {
  console.error("\nThe desktop version drifted between package.json, tauri.conf.json and Cargo.toml.");
  console.error("release-please bumps all three via `extra-files`; bump them together if you did it by hand.");
  process.exit(process.exitCode);
}
