#!/usr/bin/env node
// Bundles the PixivFlow backend into src-tauri/resources/backend so the desktop
// app can run it via `node dist/webui/index.js`.
//
// Upstream location: set $PIXIVFLOW_DIR or leave the default sibling path.
// Best-effort: if the backend has not been built, runs `npm ci && npm run build`.

import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const backendDir = path.resolve(process.env.PIXIVFLOW_DIR || path.join(root, '..', 'redtidev1918', 'PixivFlow'));
const dest = path.join(root, 'src-tauri', 'resources', 'backend');

if (!fs.existsSync(backendDir)) {
  throw new Error(`PixivFlow backend not found at ${backendDir}. Set $PIXIVFLOW_DIR.`);
}

// 1) Install + build the backend if needed.
const entry = findWebuiEntry(backendDir);
if (!entry) {
  console.log('[bundle:backend] installing deps (npm ci)…');
  execSync('npm ci', { cwd: backendDir, stdio: 'inherit' });
  console.log('[bundle:backend] building (npm run build)…');
  execSync('npm run build', { cwd: backendDir, stdio: 'inherit' });
} else {
  console.log(`[bundle:backend] found built entry at ${entry}`);
}

const outRoot = findWebuiEntry(backendDir);
if (!outRoot) {
  throw new Error('Could not locate dist/webui/index.js after build.');
}

// 2) Copy the compiled output + runtime deps into the resource dir.
fs.rmSync(dest, { recursive: true, force: true });
fs.mkdirSync(dest, { recursive: true });

fs.cpSync(outRoot, path.join(dest, 'dist'), { recursive: true });
for (const item of ['node_modules', 'package.json']) {
  const src = path.join(backendDir, item);
  if (fs.existsSync(src)) {
    fs.cpSync(src, path.join(dest, item), { recursive: true });
  }
}
console.log(`[bundle:backend] bundled backend -> ${dest}`);

function findWebuiEntry(dir) {
  for (const cand of ['dist', 'build', 'lib', 'out']) {
    const p = path.join(dir, cand);
    if (fs.existsSync(path.join(p, 'webui', 'index.js'))) return p;
  }
  if (fs.existsSync(path.join(dir, 'webui', 'index.js'))) return dir;
  return null;
}