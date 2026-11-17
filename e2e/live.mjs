// Live end-to-end check of every library: search (PL + EN), preview, downloaded code.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, realpathSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const CLI = fileURLToPath(new URL('../target/release/clayui', import.meta.url));
const work = realpathSync(mkdtempSync(path.join(os.tmpdir(), 'clayui-e2e-')));
const env = { ...process.env, HOME: work, USERPROFILE: work, CLAYUI_CACHE_DIR: path.join(work, 'cache') };
const CASES = [
  ['shadcn', 'dialog', 'dialog'],
  ['magicui', 'marquee scrolling', 'marquee'],
  ['aceternity', 'aurora background', 'aurora-background'],
  ['kokonut', 'carousel cards', 'carousel-cards'],
  ['beautifului', 'loading state', 'loading-state'],
  ['21st', 'carousel', null],
  ['rareui', 'liquid button', 'liquid-button'],
  ['base-ui', 'dialog', 'dialog'],
  ['radix', 'tooltip', 'tooltip'],
  ['ui-sdk', 'combobox', 'combobox'],
  ['sonner', 'toast', 'sonner'],
  ['vaul', 'drawer', 'vaul'],
  ['cmdk', 'command menu', 'cmdk'],
  ['number-flow', 'animated number', 'number-flow'],
];

const run = (...args) => {
  const result = spawnSync(CLI, args, { env, cwd: work, encoding: 'utf8', timeout: 120_000 });
  return { code: result.status, out: result.stdout, err: result.stderr };
};
const json = (result) => { try { return JSON.parse(result.out); } catch { return null; } };

run('sync');
const rows = [];
for (const [source, query, name] of CASES) {
  const lookup = (query) => json(run('search', query, '--source', source, '--limit', '5', '--json'))?.results ?? [];
  const found = lookup(query);
  const id = name ? `${source}/${name}` : found[0]?.id;
  const hit = (list) => list.some((item) => item.id === id);
  const shown = json(run('show', id, '--json'));
  const got = run('get', id, '--out', path.join(work, 'out'), '--json');
  const saved = json(got);
  const codeFiles = (saved?.files ?? []).filter((file) => /\.(tsx?|css)$/.test(file));
  const real = codeFiles.filter((file) => /export|function|const|\{/.test(readFileSync(path.join(saved.destination, file), 'utf8')));
  rows.push({
    source, id, search: hit(found),
    preview: shown?.previewStatus ?? 'error', license: shown?.license ?? 'unknown', gaps: (shown?.gaps ?? []).join(',') || '-',
    get: got.code, codeFiles: real.length, usage: Boolean(saved?.files?.includes('USAGE.md')), meta: saved ? existsSync(path.join(saved.destination, 'clayui.json')) : false,
  });
}
console.table(rows);
rmSync(work, { recursive: true, force: true });
const failed = rows.filter((row) => !row.search || row.get !== 0 || row.codeFiles === 0);
console.log(`Full flow: ${rows.length - failed.length}/${rows.length}`);
if (failed.length) console.log(`Incomplete: ${failed.map((row) => row.source).join(', ')}`);
process.exitCode = failed.length ? 1 : 0;
