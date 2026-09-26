#!/usr/bin/env node
// Compose the REAL PixivFlow WebUI frontend into the desktop bundle.
//
//   node scripts/fetch-pixivflow-webui.mjs [--source <dir>] [--version <x.y.z>]
//
// Why this exists: the runtime package (`pixivflow`) ships the WebUI *server*
// (`dist/webui/index.js`) but no frontend, and the committed
// `src-tauri/resources/webui/dist/index.html` is only a placeholder notice. The
// frontend is what the user actually sees, so a release build must lay it down
// from the version pinned in `desktop-manifest.json` — otherwise the installer
// ships a "STATIC_PATH" page instead of the app.
//
// Resolution, in order:
//   1. `--source <dir>`: a local pixivflow-webui checkout (its `dist/` is reused
//      when present, rebuilt when missing).
//   2. a sibling checkout (`../redtidev1918/pixivflow-webui`) whose package.json
//      version equals the pin.
//   3. a shallow clone of `v<version>` from GitHub, built in a temp dir.
//
// Output (git-ignored assets under a tracked index.html): the checkout's `dist/`
// is copied over `src-tauri/resources/webui/dist/`, replacing the placeholder.
//
// Requires: node >= 18, npm, git, network.

import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const DEST = join(ROOT, 'src-tauri', 'resources', 'webui', 'dist');
const REPO = 'https://github.com/redtidev1918/pixivflow-webui.git';

const fail = (m) => { console.error('✖ ' + m); process.exit(1); };
const log = (m) => console.log('• ' + m);
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

function argValue(name) {
  const a = process.argv.slice(2);
  const i = a.indexOf(`--${name}`);
  return i >= 0 ? a[i + 1] : undefined;
}

function run(cmd, args, opts = {}) {
  log([cmd, ...args].join(' '));
  execFileSync(cmd, args, { stdio: 'inherit', env: process.env, ...opts });
}

const pinned = () => {
  const manifest = readJson(join(ROOT, 'desktop-manifest.json'));
  const version = manifest?.components?.['pixivflow-webui']?.version;
  if (!version) fail('desktop-manifest.json has no components.pixivflow-webui.version to pin the WebUI to');
  return version;
};

function isWebuiCheckout(dir) {
  const pj = join(dir, 'package.json');
  return existsSync(pj) && readJson(pj).name === 'pixivflow-webui';
}

function ensureBuiltDist(dir) {
  const dist = join(dir, 'dist');
  if (!(existsSync(dist) && existsSync(join(dist, 'index.html')))) {
    log(`no built dist/ in ${dir}; building it`);
    run('npm', ['ci', '--legacy-peer-deps'], { cwd: dir });
    run('npm', ['run', 'build'], { cwd: dir });
  }
  if (!(existsSync(dist) && existsSync(join(dist, 'index.html')))) {
    fail(`build did not produce dist/index.html in ${dir}`);
  }
  return dist;
}

const version = argValue('version') || pinned();
const source = argValue('source');
let dist;
let temporary = null;

if (source) {
  const dir = resolve(source);
  if (!isWebuiCheckout(dir)) fail(`--source ${dir} is not a pixivflow-webui checkout`);
  log(`using --source checkout ${dir}`);
  dist = ensureBuiltDist(dir);
} else {
  const sibling = resolve(ROOT, '..', 'redtidev1918', 'pixivflow-webui');
  if (isWebuiCheckout(sibling) && readJson(join(sibling, 'package.json')).version === version) {
    log(`using sibling checkout ${sibling} (v${version})`);
    dist = ensureBuiltDist(sibling);
  } else {
    temporary = mkdtempSync(join(tmpdir(), 'pfx-webui-'));
    log(`cloning ${REPO} at v${version} -> ${temporary}`);
    run('git', ['clone', '--depth', '1', '--branch', `v${version}`, REPO, temporary]);
    dist = ensureBuiltDist(temporary);
  }
}

const assets = join(dist, 'assets');
if (!(existsSync(assets) && readdirSync(assets).length > 0)) {
  fail(`the WebUI dist at ${dist} has no assets/ — refusing to bundle a frontend that cannot render`);
}
const html = readFileSync(join(dist, 'index.html'), 'utf8');
if (html.includes('STATIC_PATH')) {
  fail('the WebUI dist is still the committed placeholder — the real frontend was not built');
}

rmSync(DEST, { recursive: true, force: true });
cpSync(dist, DEST, { recursive: true });
log(`✓ composed PixivFlow WebUI v${version} -> ${DEST}`);

if (temporary) rmSync(temporary, { recursive: true, force: true });
