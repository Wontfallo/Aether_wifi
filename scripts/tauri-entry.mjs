import { spawn } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const isLinuxDev = process.platform === 'linux' && args[0] === 'dev';
const bypassLauncher = process.env.AETHER_BYPASS_LAUNCHER === '1';

const command = isLinuxDev && !bypassLauncher
  ? 'bash'
  : path.join(
      projectRoot,
      'node_modules',
      '.bin',
      process.platform === 'win32' ? 'tauri.cmd' : 'tauri',
    );

const commandArgs = isLinuxDev && !bypassLauncher
  ? [path.join(projectRoot, 'aether.sh')]
  : args;

const child = spawn(command, commandArgs, {
  cwd: projectRoot,
  env: process.env,
  stdio: 'inherit',
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

child.on('error', (error) => {
  console.error(error);
  process.exit(1);
});
