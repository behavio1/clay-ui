#!/usr/bin/env node
// Runs the prebuilt native binary for this machine. npm installs only the matching platform package.
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';

const PACKAGES = {
  'darwin-arm64': 'clayui-darwin-arm64',
  'darwin-x64': 'clayui-darwin-x64',
  'linux-x64': 'clayui-linux-x64',
  'linux-arm64': 'clayui-linux-arm64',
  'win32-x64': 'clayui-win32-x64',
};

const platform = `${process.platform}-${process.arch}`;
const name = PACKAGES[platform];
if (!name) {
  console.error(`Clay UI has no prebuilt binary for ${platform}. Supported: ${Object.keys(PACKAGES).join(', ')}.`);
  process.exit(1);
}

let binary;
try {
  const manifest = createRequire(import.meta.url).resolve(`${name}/package.json`);
  binary = path.join(path.dirname(manifest), process.platform === 'win32' ? 'clayui.exe' : 'clayui');
} catch {
  console.error(`The package ${name} is missing. Reinstall without --no-optional, or run: npm install ${name}`);
  process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' });
const signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
const handlers = new Map(signals.map(signal => [signal, () => { child.kill(signal); }]));
for (const [signal, handler] of handlers) process.on(signal, handler);
child.on('error', error => {
  console.error(`Could not start native Clay UI: ${error.message}`);
  process.exitCode = 1;
});
child.on('exit', (code, signal) => {
  for (const [name, handler] of handlers) process.off(name, handler);
  if (signal) process.kill(process.pid, signal);
  else process.exitCode = code ?? 1;
});
