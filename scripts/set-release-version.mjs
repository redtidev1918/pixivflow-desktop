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

const args = process.argv.slice(2);
const version = args[0];
const dryRun = args.includes("--dry-run");
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error(`usage: node scripts/set-release-version.mjs <semver> [--dry-run]  (got: ${version ?? "<missing>"})`);
  process.exit(2);
}

const root = process.cwd();

// Any release still needs its Chinese notes on disk; bake that into this cut so
// a hand-cut cannot get past the version bump and then stall on release notes.
const notesPath = join(root, ".github", "release-notes", `${version}.md`);
let hasNotes = false;
try {
  const notes = readFileSync(notesPath, "utf8");
  hasNotes = /本次更新/.test(notes) && /[\u4e00-\u9fff]/.test(notes);
} catch {}
const missingNotes = !hasNotes;

const reads = {};
const updates = [
  ["package.json", (c) => c.includes(`"version": "${version}"`)],
  ["src-tauri/tauri.conf.json", (c) => c.includes(`"version": "${version}"`)],
  [
    "src-tauri/Cargo.toml",
    (c) => new RegExp(`^version\\s*=\\s*"${version.replace(/\./g, "\\.")}"`, "m").test(c),
  ],
];

const writes = [];
for (const [rel, alreadyAt] of updates) {
  const abs = join(root, rel);
  const content = readFileSync(abs, "utf8");
  reads[rel] = content;
  if (alreadyAt(content)) continue;
  if (dryRun) {
    writes.push(rel);
    continue;
  }
  const next =
    rel === "src-tauri/Cargo.toml"
      ? content.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`)
      : content.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
  writeFileSync(abs, next);
  writes.push(rel);
}

// A dry run must be read-only AND must flag every condition that would make the
// real cut fail, so the operator can fix them before running it for real.
if (dryRun) {
  console.log(`DRY RUN — would set ${version} in: ${writes.length ? writes.join(", ") : "(all already current)"}`);
  if (missingNotes) {
    console.log(`DRY RUN ✗ release notes ${notesPath} is missing or lacks the 本次更新 heading / Chinese content`);
  } else {
    console.log(`DRY RUN ✓ release notes ${notesPath} present`);
  }
  process.exit(missingNotes ? 1 : 0);
}

// Post-write validation, mirroring the release-metadata CI gate so a hand-cut
// passes the same bar a real PR would.
if (missingNotes) {
  console.error(`✗ ${notesPath} is missing or lacks the 本次更新 heading / Chinese content`);
  process.exitCode = 1;
}
const parseJson = (rel) => JSON.parse(readFileSync(join(root, rel), "utf8")).version;
if (parseJson("package.json") !== version) {
  console.error(`✗ package.json did not take ${version}`);
  process.exitCode = 1;
}
if (parseJson("src-tauri/tauri.conf.json") !== version) {
  console.error(`✗ src-tauri/tauri.conf.json did not take ${version}`);
  process.exitCode = 1;
}
const inCargo = /^version\s*=\s*"([^"]+)"/m.exec(readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8"))?.[1];
if (inCargo !== version) {
  console.error(`✗ src-tauri/Cargo.toml did not take ${version}`);
  process.exitCode = 1;
}

if (writes.length === 0) {
  console.log(`all version files already say ${version}`);
} else {
  console.log(`set ${version} in: ${writes.join(", ")}`);
}
if (process.exitCode) process.exit(process.exitCode);
console.log("✓ three version files + release notes are consistent");
