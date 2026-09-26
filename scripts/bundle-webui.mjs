#!/usr/bin/env node
// Bundles the built pixivflow-webui static SPA into src-tauri/resources/webui.
// The local PixivFlow backend serves this directory via STATIC_PATH, making the
// WebUI + /api + /socket.io all same-origin under the desktop window.
//
// Upstream location: set $PIXIVFLOW_WEBUI_DIR or leave the default sibling path.

import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const webuiDir = path.resolve(process.env.PIXIVFLOW_WEBUI_DIR || path.join(root, '..', 'pixivflow-webui'));
const dest = path.join(root, 'src-tauri', 'resources', 'webui');

if (!fs.existsSync(webuiDir)) {
  throw new Error(`pixivflow-webui not found at ${webuiDir}. Set $PIXIVFLOW_WEBUI_DIR.`);
}

const srcDist = path.join(webuiDir, 'dist');
if (!fs.existsSync(srcDist) || !fs.existsSync(path.join(srcDist, 'index.html'))) {
  if (!fs.existsSync(path.join(webuiDir, 'node_modules'))) {
    console.log('[bundle:webui] installing deps (npm ci)…');
    execSync('npm ci', { cwd: webuiDir, stdio: 'inherit' });
  }
  console.log('[bundle:webui] building (npm run build)…');
  execSync('npm run build', { cwd: webuiDir, stdio: 'inherit' });
}

if (!fs.existsSync(path.join(srcDist, 'index.html'))) {
  throw new Error('pixivflow-webui build produced no dist/index.html.');
}

fs.rmSync(dest, { recursive: true, force: true });
fs.mkdirSync(dest, { recursive: true });
fs.cpSync(srcDist, dest, { recursive: true });
console.log(`[bundle:webui] bundled webui -> ${dest}`);