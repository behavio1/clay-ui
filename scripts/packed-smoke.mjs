// Exercise packed binaries through the real launcher; all writes stay in a temporary sandbox.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, realpathSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('..', import.meta.url));
const key = process.argv[2] ?? `${process.platform}-${process.arch}`;
const folder = path.resolve(process.argv[3] ?? `dist-npm/clayui-${key}`);
const work = realpathSync(mkdtempSync(path.join(os.tmpdir(), 'clayui-packed-')));
const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !/KEY|TOKEN|SECRET|CLAYUI/.test(k)));
Object.assign(env, { HOME: work, USERPROFILE: work, CLAYUI_CACHE_DIR: path.join(work, 'cache') });
const runtime = process.env.CLAYUI_SMOKE_NODE ?? process.execPath;
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
function run(command, args, cwd = work) {
  const p = spawnSync(command, args, { cwd, env, encoding: 'utf8', shell: process.platform === 'win32' && command === npm });
  assert.equal(p.status, 0, `${command}: ${p.error?.message ?? ''}\n${p.stderr}`);
  return p.stdout;
}
try {
  assert.deepEqual(readFileSync(path.join(folder, 'THIRD_PARTY_NOTICES.md')), readFileSync(path.join(root, 'THIRD_PARTY_NOTICES.md')));
  const binary = readFileSync(path.join(folder, process.platform === 'win32' ? 'clayui.exe' : 'clayui'));
  for (const text of ['/Users/', 'CLAYUI_TEST_BASE_URL', 'CLAYUI_TEST_TIMEOUT_MS']) assert.equal(binary.includes(Buffer.from(text)), false, `release contains ${text}`);
  const pack = dir => JSON.parse(run(npm, ['pack', dir, '--pack-destination', work, '--json']))[0].filename;
  const native = pack(folder);
  const launcher = pack(root);
  run(npm, ['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', path.join(work, native), path.join(work, launcher)]);
  const cli = path.join(work, 'node_modules', '@behavio1', 'clayui', 'bin', 'launcher.mjs');
  assert.equal(run(runtime, [cli, '--version']).trim(), JSON.parse(readFileSync(path.join(root,'package.json'))).version);
  assert.match(run(runtime, [cli, '--help']), /clayui search/);
  run(runtime, [cli, 'init', '--no-sync', '--target', 'agents']);
  assert.deepEqual(readFileSync(path.join(work, '.agents/skills/clay-ui/SKILL.md')), readFileSync(path.join(root, 'skills/clay-ui/SKILL.md')));
  console.log(`PASS packed ${key}: notices, release scan, both tarballs, clean install, launcher version/help/init`);
} finally { rmSync(work, { recursive: true, force: true }); }
