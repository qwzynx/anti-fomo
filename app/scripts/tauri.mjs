// Runs the Tauri CLI with Cargo on PATH. rustup adds ~/.cargo/bin to PATH,
// but a launcher (IDE, run button, terminal) started before Rust was
// installed keeps its old environment, and `tauri dev` then fails with
// "failed to run 'cargo metadata' ... program not found".
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';

const cargoBin = join(process.env.CARGO_HOME || join(homedir(), '.cargo'), 'bin');
// Windows spells it Path; Node keeps the original casing in process.env.
const key = Object.keys(process.env).find((k) => k.toUpperCase() === 'PATH') ?? 'PATH';
const parts = (process.env[key] ?? '').split(delimiter);
if (existsSync(cargoBin) && !parts.includes(cargoBin)) {
  process.env[key] = [cargoBin, ...parts].join(delimiter);
}

const cli = createRequire(import.meta.url).resolve('@tauri-apps/cli/tauri.js');
const run = spawnSync(process.execPath, [cli, ...process.argv.slice(2)], { stdio: 'inherit' });
process.exit(run.status ?? 1);
