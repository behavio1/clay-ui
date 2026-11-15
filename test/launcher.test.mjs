import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  test(`launcher forwards ${signal} and preserves child termination`, { skip: process.platform === 'win32' }, async () => {
    const work = mkdtempSync(path.join(os.tmpdir(), 'clayui-launcher-'));
    const folder = path.join(work, 'node_modules', `clayui-${process.platform}-${process.arch}`);
    mkdirSync(folder, { recursive: true });
    writeFileSync(path.join(folder, 'package.json'), '{}');
    const marker = path.join(work, 'ready');
    const stopped = path.join(work, 'stopped');
    writeFileSync(path.join(folder, 'clayui'), `#!${process.execPath}\nimport('node:fs').then(fs => { process.on('${signal}', () => { fs.writeFileSync(${JSON.stringify(stopped)}, '${signal}'); process.exit(37); }); fs.writeFileSync(${JSON.stringify(marker)}, String(process.pid)); setInterval(()=>{},1000); });\n`, { mode: 0o755 });
    copyFileSync(new URL('../bin/launcher.mjs', import.meta.url), path.join(work, 'launcher.mjs'));
    const child = spawn(process.execPath, [path.join(work, 'launcher.mjs')], { env: { PATH: process.env.PATH, HOME: work, USERPROFILE: work }, stdio: 'ignore' });
    const exit = once(child, 'exit');
    try {
      let ready = false;
      for (let n = 0; n < 100; n++) { try { readFileSync(marker); ready = true; break; } catch {} await pause(10); }
      assert.equal(ready, true);
      child.kill(signal);
      const [code, received] = await Promise.race([exit, pause(3000).then(() => { throw new Error('launcher did not exit'); })]);
      assert.equal(code, 37); assert.equal(received, null);
      assert.equal(readFileSync(stopped, 'utf8'), signal);
    } finally { child.kill('SIGKILL'); rmSync(work, { recursive: true, force: true }); }
  });
}
