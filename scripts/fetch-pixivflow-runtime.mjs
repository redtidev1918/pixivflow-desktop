#!/usr/bin/env node
// F2.3 — acquire the REAL PixivFlow backend runtime into the desktop bundle.
//
//   node scripts/fetch-pixivflow-runtime.mjs [--source <dir|npm-spec>]
//
// --source
//   * path to a local PixivFlow checkout (a dir whose package.json name is
//     `pixivflow`). Prefers its existing built `dist/`; rebuilds if missing.
//   * an npm spec, e.g. `pixivflow@2.46.0` (fetched via `npm pack`).
//   * omitted => auto-discover a sibling checkout (../redtidev1918/PixivFlow).
//
// Output (git-ignored build products):
//   src-tauri/resources/runtime/pixivflow/{dist/, node_modules/, package.json, VERSION}
// Rewrites in place, locally (a working artifact, not committed):
//   src-tauri/resources/runtime/pixivflow/runtime-manifest.json  -> real entry
// Leaves dev-backend.mjs (the committed dev fallback) untouched, so a fresh
// clone still defaults to the lightweight dev stand-in until this runs.
//
// Requires: node >= 18, npm, network (npm honours HTTPS_PROXY when set).

import { execSync } from 'node:child_process';
import {
  chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync,
  realpathSync, renameSync, rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const RUNTIME_DIR = join(ROOT, 'src-tauri', 'resources', 'runtime', 'pixivflow');
const MANIFEST = join(RUNTIME_DIR, 'runtime-manifest.json');

const fail = (m) => { console.error('✖ ' + m); process.exit(1); };
const log = (m) => console.log('• ' + m);

function argValue(name) {
  const a = process.argv.slice(2);
  const i = a.indexOf(`--${name}`);
  return i >= 0 ? a[i + 1] : undefined;
}

const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

const platformTag = () => `${process.platform}-${process.arch}`;

function run(cmd, opts = {}) {
  log(cmd);
  execSync(cmd, { stdio: 'inherit', env: process.env, ...opts });
}

function resolveSource(spec, baseDir) {
  if (!spec) {
    const cands = [
      resolve(baseDir, '..', 'redtidev1918', 'PixivFlow'),
      resolve(baseDir, '..', 'PixivFlow'),
      resolve(baseDir, 'PixivFlow'),
    ];
    for (const c of cands) {
      const pj = join(c, 'package.json');
      if (existsSync(pj) && readJson(pj).name === 'pixivflow') return { kind: 'dir', path: c };
    }
    fail('no --source given and no sibling PixivFlow checkout found');
  }
  if (existsSync(spec)) return { kind: 'dir', path: resolve(spec) };
  return { kind: 'npm', spec };
}

function ensureBuiltDist(checkoutDir) {
  const dist = join(checkoutDir, 'dist');
  if (existsSync(dist) && readdirHas(dist, 'webui')) {
    return;
  }
  log('no ready dist/, building …');
  run(`npm ci --no-audit --no-fund`, { cwd: checkoutDir });
  run(`npm run build`, { cwd: checkoutDir });
  if (!(existsSync(dist) && readdirHas(dist, 'webui'))) {
    fail('build did not produce dist/webui (entry dist/webui/index.js)');
  }
}

function readdirHas(dir, name) {
  try { return readdirSync(dir).includes(name); } catch { return false; }
}

function installProdDeps(pkgJsonPath, lockPath, destDir) {
  // Assemble prod-only node_modules in an isolated staging dir.
  const stage = mkdtempSync(join(tmpdir(), 'pfx-prod-'));
  try {
    cpSync(pkgJsonPath, join(stage, 'package.json'));
    if (lockPath && existsSync(lockPath)) {
      cpSync(lockPath, join(stage, 'package-lock.json'));
    }
    run(`npm ci --omit=dev --no-audit --no-fund`, { cwd: stage });
    // Replace any previous tree instead of merging into it, so a stale entry
    // from an earlier fetch can never survive into the next bundle.
    rmSync(join(destDir, 'node_modules'), { recursive: true, force: true });
    cpSync(join(stage, 'node_modules'), join(destDir, 'node_modules'), { recursive: true });
    materializeSymlinks(join(destDir, 'node_modules'));
    log(`node_modules copied (prod-only) -> ${join(destDir, 'node_modules')}`);
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}

// Collapse every symlink in the copied tree into a real file/directory.
//
// `npm ci` runs in a temp stage, so `.bin` entries copied out of it point at
// deleted `/private/var/folders/.../T/pfx-prod-*` paths. The Tauri build script
// walks every bundled resource and rejects a symlink that does not resolve
// inside the resource dir ("resource path … doesn't exist"), which breaks BOTH
// `tauri build` and `cargo test`. Dangling links are dropped; surviving links
// (e.g. `.bin/x -> ../x/bin/x.js`) are dereferenced so the resource tree is
// self-contained.
function materializeSymlinks(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isSymbolicLink()) {
      if (!existsSync(p)) {
        rmSync(p, { force: true });
        continue;
      }
      const target = realpathSync(p);
      const tmp = `${p}.__materialized__`;
      cpSync(target, tmp, { recursive: true, dereference: true });
      rmSync(p, { recursive: true, force: true });
      renameSync(tmp, p);
    } else if (entry.isDirectory()) {
      materializeSymlinks(p);
    }
  }
}

// Materialize npm-workspace deps that the built dist requires but that are NOT
// published to the registry (e.g. @redtidev/pixiv-client, whose package lives
// under `packages/` and is linked into node_modules as a symlink). npm ci in a
// staged dir leaves that symlink dangling (it points at the deleted stage), so
// we replace it with a real copy of the compiled package from the source root.
function materializeWorkspaceDeps(sourceRoot, destNodeModules) {
  if (!sourceRoot) return;
  const scoped = join(sourceRoot, 'node_modules', '@redtidev');
  if (!existsSync(scoped)) return;
  const outScoped = join(destNodeModules, '@redtidev');
  mkdirSync(outScoped, { recursive: true });
  for (const name of readdirSync(scoped)) {
    const link = join(scoped, name);
    const real = (() => { try { return realpathSync(link); } catch { return null; } })();
    if (!real || !existsSync(join(real, 'package.json'))) continue;
    const out = join(outScoped, name);
    // npm ci left a real OR dangling-symlink entry here; remove it either way.
    rmSync(out, { recursive: true, force: true });
    cpSync(real, out, { recursive: true });
    log(`workspace dep materialized: @redtidev/${name}`);
  }
}

// Copy the running standalone node executable into the runtime dir and return
// its manifest-relative command. `process.execPath` is resolved first so a
// version-manager shim or wrapper doesn't get copied.
function bundleNodeBinary(destDir) {
  const isWin = process.platform === 'win32';
  let real = process.execPath;
  try { real = realpathSync(process.execPath); } catch {}
  const name = isWin ? 'node.exe' : 'node';
  cpSync(real, join(destDir, name));
  if (!isWin) chmodSync(join(destDir, name), 0o755);
  log(`standalone node bundled -> ${name} (${process.version})`);
  return `./${name}`;
}

// --- main -----------------------------------------------------------------
const srcSpec = argValue('source');
const src = resolveSource(srcSpec, __dirname);
const workDir = existsSync(RUNTIME_DIR) ? RUNTIME_DIR : fail(`missing ${RUNTIME_DIR}`);

let pkgDir, pkg;
if (src.kind === 'dir') {
  const pj = join(src.path, 'package.json');
  if (!existsSync(pj)) fail(`not a package dir: ${src.path}`);
  pkg = readJson(pj);
  if (pkg.name !== 'pixivflow') fail(`expected a pixivflow package, got ${pkg.name}`);
  ensureBuiltDist(src.path);
  pkgDir = src.path;
} else {
  // npm spec: pack + extract into a temp dir.
  const stage = mkdtempSync(join(tmpdir(), 'pfx-pull-'));
  try {
    const tarball = execSync(`npm pack ${src.spec} --pack-destination ${stage} --no-audit --no-fund 2>&1`, { encoding: 'utf8' })
      .trim().split('\n').pop().trim();
    const pdir = join(stage, 'package');
    mkdirSync(pdir, { recursive: true });
    execSync(`tar -xzf ${join(stage, tarball)} -C ${pdir} --strip-components=1`, { stdio: 'inherit' });
    const pj = join(pdir, 'package.json');
    if (!existsSync(pj)) fail('npm pack did not contain package.json');
    pkg = readJson(pj);
    pkgDir = pdir;
  } catch (e) {
    fail(`npm pack failed: ${e.stderr || e.message}`);
  }
}

const version = pkg.version;
const nodeBin = bundleNodeBinary(workDir);
const manifest = {
  name: 'pixivflow',
  version,
  platform: platformTag(),
  // Invoke the runtime-local standalone node, not a PATH lookup: a PATH "node"
  // may resolve to a foreign shim (which can reparent or kill the child), and a
  // user machine may have no node at all. This keeps the runtime self-contained.
  command: [nodeBin],
  args: ['./dist/webui/index.js'],
  health: '/api/health',
  // real backend finds its own fallback webui-frontend; desktop prefers its
  // bundled dist, so no staticPath here (desktop fallback applies).
  servesWebui: true,
};

log(`laying out ${pkg.name}@${version} (${platformTag()}) -> ${workDir}`);
// fresh real-runtime artifact (dist + prod node_modules), keep dev-backend.mjs.
for (const sub of ['dist', 'node_modules']) {
  rmSync(join(workDir, sub), { recursive: true, force: true });
}
cpSync(join(pkgDir, 'dist'), join(workDir, 'dist'), { recursive: true });
cpSync(join(pkgDir, 'package.json'), join(workDir, 'package.json'));
writeFileSync(join(workDir, 'VERSION'), version + '\n');
writeFileSync(join(workDir, 'runtime-manifest.json'), JSON.stringify(manifest, null, 2) + '\n');

installProdDeps(join(pkgDir, 'package.json'), join(pkgDir, 'package-lock.json'), workDir);

// restore workspace-local deps (local checkouts only) so dist is self-contained
const sourceRoot = src.kind === 'dir' ? src.path : null;
materializeWorkspaceDeps(sourceRoot, join(workDir, 'node_modules'));

log(`✓ bundled runtime: ${pkg.name}@${version}`);
log(`✓ manifest command: ${manifest.command.join(' ')}`);
log(`  (desktop discovery now resolves source=bundled, exe=${join(workDir, 'dist', 'webui', 'index.js')})`);
log('note: runtime-manifest.json is a local working artifact; a fresh `git checkout` restores the dev stand-in.');