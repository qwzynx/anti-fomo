// Runs before `npm run dev` / `build` / `tauri`. The repo is shared between
// Linux and Windows, and npm only installs the native binaries (Tauri CLI,
// esbuild, rolldown/rollup, lightningcss, tailwind oxide) for the OS that ran
// `npm install`. Switching OS then fails with "Cannot find native binding".
// If node_modules was installed for another platform, reinstall it here.
// Uses only Node built-ins: node_modules may be the thing that is broken.
import { existsSync, readdirSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const root = process.cwd();
const nm = join(root, 'node_modules');
const tag = `${process.platform}-${process.arch}`;

// [directory under node_modules, name prefix of its per-platform packages]
const natives = [
  ['@tauri-apps', 'cli-'],
  ['@esbuild', ''],
  ['@rolldown', 'binding-'],
  ['@rollup', 'rollup-'],
  ['@tailwindcss', 'oxide-'],
  ['.', 'lightningcss-'],
];
const anyPlatform = /^(linux|win32|darwin|android|freebsd|openbsd|sunos|aix)-/;

function missing() {
  if (!existsSync(nm)) return 'node_modules is missing';
  for (const [dir, family] of natives) {
    const path = join(nm, dir);
    if (!existsSync(path)) continue;
    const builds = readdirSync(path)
      .filter((e) => e.startsWith(family) && anyPlatform.test(e.slice(family.length)));
    // Only check families that have a native build installed for some OS.
    if (builds.length && !builds.some((e) => e.startsWith(family + tag))) {
      return `${dir === '.' ? '' : dir + '/'}${family}${tag}* is not installed`;
    }
  }
  return null;
}

const reason = missing();
if (reason) {
  console.log(`[deps] ${reason} — node_modules was installed for another OS.`);
  console.log(`[deps] Reinstalling dependencies for ${tag} (one-time per OS switch)...`);
  rmSync(nm, { recursive: true, force: true });
  const npm = spawnSync('npm ci --no-audit --no-fund', { cwd: root, stdio: 'inherit', shell: true });
  if (npm.status !== 0) process.exit(npm.status ?? 1);
}
