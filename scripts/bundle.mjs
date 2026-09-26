#!/usr/bin/env node
// Shorthand for bundling both upstreams into the desktop resources.

import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
for (const script of ['bundle-webui.mjs', 'bundle-backend.mjs']) {
  execSync(`node ${path.join(root, 'scripts', script)}`, { stdio: 'inherit' });
}
console.log('[bundle] done.');