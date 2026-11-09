import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, symlinkSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { rank, rankSemantic, expandQuery } from '../src/search.mjs';
import { safeRelative } from '../src/project.mjs';
import { readKey } from '../src/embeddings.mjs';

const CLI = fileURLToPath(new URL('../bin/clayui.mjs', import.meta.url));
const MOCK = new URL('../test-helpers/mock-fetch.mjs', import.meta.url).href;
const temporaryFolders = [];
const temp = () => { const p = mkdtempSync(path.join(os.tmpdir(), 'clayui-test-')); temporaryFolders.push(p); return p; };
process.on('exit', () => { for (const p of temporaryFolders) rmSync(p, { recursive: true, force: true }); });
const item = (source, name, extra = {}) => ({ id: `${source}/${name}`, source, name, title: name, description: '', gaps: [], ...extra });

function cli(args, { mock = 'timeout', cache = temp(), env = {} } = {}) {
  const result = spawnSync(process.execPath, ['--import', MOCK, CLI, ...args], {
    encoding: 'utf8',
    env: { PATH: process.env.PATH, HOME: temp(), CLAYUI_CACHE_DIR: cache, CLAYUI_MOCK: mock, ...env },
  });
  return { code: result.status, out: result.stdout, err: result.stderr, cache };
}

const seedIndex = (cache, source, items, extra = {}) => writeFileSync(path.join(cache, 'index.json'), JSON.stringify({ sources: { [source]: { items, syncedAt: new Date().toISOString(), error: null, ...extra } } }));

test('S1/S2 English words expand to component synonyms', () => {
  assert.deepEqual(expandQuery('carousel')[0].synonyms, ['slider']);
  assert.equal(rank([item('a', 'carousel'), item('a', 'tabs')], 'carousel')[0].name, 'carousel');
});

test('S6 no hits gives an empty result and exit 0', () => {
  const cache = temp();
  seedIndex(cache, 'shadcn', [item('shadcn', 'dialog')]);
  const result = cli(['search', 'qqqzzz', '--source', 'shadcn', '--json'], { cache });
  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.out).results, []);
});

test('S4 a timing-out source keeps its last good data and reports the error', () => {
  const cache = temp();
  seedIndex(cache, 'shadcn', [item('shadcn', 'dialog')]);
  const result = cli(['sync', '--source', 'shadcn'], { cache });
  assert.equal(result.code, 0);
  const entry = JSON.parse(readFileSync(path.join(cache, 'index.json'))).sources.shadcn;
  assert.equal(entry.items.length, 1);
  assert.match(entry.error, /ETIMEDOUT/);
});

test('S5 invalid JSON does not overwrite the index', () => {
  const cache = temp();
  seedIndex(cache, 'magicui', [item('magicui', 'marquee')]);
  cli(['sync', '--source', 'magicui'], { cache, mock: 'badjson' });
  const entry = JSON.parse(readFileSync(path.join(cache, 'index.json'))).sources.magicui;
  assert.equal(entry.items[0].name, 'marquee');
  assert.match(entry.error, /invalid JSON/);
});

test('S8 a missing 21st tag skips 21st without failing the search', () => {
  const cache = temp();
  seedIndex(cache, 'shadcn', [item('shadcn', 'dialog')]);
  const result = cli(['search', 'dialog', '--json'], { cache, mock: 'notag' });
  assert.equal(result.code, 0);
  const body = JSON.parse(result.out);
  assert.equal(body.results[0].id, 'shadcn/dialog');
  assert.ok(body.skipped.some((entry) => entry.source === '21st'));
});

test('S18 21st without a key: get exits 3 and writes nothing', () => {
  const cache = temp();
  const out = temp();
  const fetchMarkdown = new URL('data:text/javascript,' + encodeURIComponent("globalThis.fetch = async () => new Response('# X\\n\\n> d\\n\\n- License: mit\\n- Live preview: https://21st.dev/@a/components/b\\n')"));
  const result = spawnSync(process.execPath, ['--import', fetchMarkdown.href, CLI, 'get', '21st/a/b', '--out', out], { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: temp(), CLAYUI_CACHE_DIR: cache } });
  assert.equal(result.status, 3);
  assert.deepEqual(readdirSync(out), []);
});

test('S19 paths leaving the example folder are rejected', () => {
  for (const unsafe of ['../../x', '/etc/passwd', '~/x', 'C:\\x', 'a/../../b', 'a/./b']) assert.equal(safeRelative(unsafe), null, unsafe);
  assert.equal(safeRelative('components/ui/a.tsx'), 'components/ui/a.tsx');
});

test('S20 malformed ids fail before any network call', () => {
  for (const id of ['magicui/../secret', 'magicui/-rf', 'nope']) {
    const log = path.join(temp(), 'calls.log');
    const result = cli(['show', id], { env: { CLAYUI_MOCK_LOG: log } });
    assert.equal(result.code, 2, id);
    assert.throws(() => readFileSync(log));
  }
});

test('S21 --project without package.json exits 2', () => {
  assert.equal(cli(['show', 'shadcn/dialog', '--project', temp()]).code, 2);
});

test('S23 unknown command or source exits 2; install commands do not exist', () => {
  assert.equal(cli(['frobnicate']).code, 2);
  assert.equal(cli(['add', 'shadcn/dialog']).code, 2);
  assert.equal(cli(['search', 'x', '--source', 'nope']).code, 2);
});

test('S22 secrets never reach stdout, stderr or the cache', () => {
  const cache = temp();
  seedIndex(cache, 'shadcn', [item('shadcn', 'dialog')]);
  const log = path.join(temp(), 'calls.log');
  const secrets = { API_KEY_21ST: 'TEST_KEY_123', GEMINI_API_KEY: 'TEST_GEMINI_456', CLAYUI_MOCK_LOG: log };
  const result = cli(['search', 'dialog', '--json'], { cache, env: secrets });
  const written = readdirSync(cache).map((file) => readFileSync(path.join(cache, file), 'utf8')).join('');
  for (const leak of ['TEST_KEY_123', 'TEST_GEMINI_456']) {
    assert.ok(!result.out.includes(leak) && !result.err.includes(leak) && !written.includes(leak), leak);
  }
  for (const line of readFileSync(log, 'utf8').split('\n').filter(Boolean)) assert.ok(!line.split(' ')[0].includes('TEST_'), line);
});

test('Gemini failure falls back to token search with a warning', () => {
  const cache = temp();
  seedIndex(cache, 'shadcn', [item('shadcn', 'dialog')]);
  const body = JSON.parse(cli(['search', 'dialog', '--source', 'shadcn', '--json'], { cache, env: { GEMINI_API_KEY: 'x' } }).out);
  assert.equal(body.results[0].id, 'shadcn/dialog');
  assert.ok(body.warnings.some((warning) => /semantic/i.test(warning)));
});

test('the key is read from the env file only as GOOGLE_API_KEY', () => {
  const file = path.join(temp(), '.env');
  writeFileSync(file, 'OTHER=1\nGOOGLE_API_KEY="abc"\n');
  assert.equal(readKey({ CLAYUI_GEMINI_ENV_FILE: file }), 'abc');
  assert.equal(readKey({ CLAYUI_GEMINI_ENV_FILE: '/nonexistent' }), null);
});

test('semantic order follows cosine; small nudges only separate near ties', () => {
  const candidates = [item('a', 'toast', { gaps: ['needs-key'] }), item('b', 'toast'), item('c', 'menu', { gaps: [] })];
  const similarity = new Map([['a/toast', 0.8], ['b/toast', 0.79], ['c/menu', 0.2]]);
  assert.equal(rankSemantic(candidates, similarity, { gapPenalty: 0, curatedBonus: 0 })[0].id, 'a/toast');
  assert.equal(rankSemantic(candidates, similarity, { gapPenalty: 0.02 })[0].id, 'b/toast');
  assert.equal(rankSemantic(candidates, similarity, { gapPenalty: 0.02, minSimilarity: 0 }).at(-1).id, 'c/menu');
  assert.equal(rankSemantic(candidates, similarity, { gapPenalty: 0.02, minSimilarity: 0.6 }).length, 2);
});

test('BM25 finds a multi-word description by any matching word', () => {
  const candidates = [item('a', 'command', { description: 'palette' }), item('b', 'input')];
  assert.equal(rank(candidates, 'palette of commands')[0].id, 'a/command');
});

test('a missing demo page or maintenance note never changes the order', () => {
  const candidates = [item('a', 'cmdk', { gaps: ['no-preview', 'unmaintained'] }), item('b', 'other')];
  const similarity = new Map([['a/cmdk', 0.8], ['b/other', 0.79]]);
  assert.equal(rankSemantic(candidates, similarity, { gapPenalty: 0.5, curatedBonus: 0 })[0].id, 'a/cmdk');
});

const registryMock = (body) => new URL('data:text/javascript,' + encodeURIComponent(`globalThis.fetch = async () => new Response(${JSON.stringify(JSON.stringify(body))})`)).href;
const raw = (script, args, cache, env = {}) => spawnSync(process.execPath, ['--import', script, CLI, ...args], { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: temp(), CLAYUI_CACHE_DIR: cache, ...env } });
const magicItem = () => item('magicui', 'button', { previewUrl: 'https://x.dev/a' });

test('R01 get --force refuses to write through symlinks', () => {
  const cache = temp();
  const out = temp();
  const outside = temp();
  seedIndex(cache, 'magicui', [magicItem()]);
  mkdirSync(path.join(out, 'magicui', 'button'), { recursive: true });
  symlinkSync(outside, path.join(out, 'magicui', 'button', 'nested'));
  const result = raw(registryMock({ files: [{ path: 'nested/button.tsx', content: 'export const a = 1' }] }), ['get', 'magicui/button', '--out', out, '--force'], cache);
  assert.equal(result.status, 1);
  assert.deepEqual(readdirSync(outside), []);
});

test('R01 a symlinked final file or clayui.json is not followed', () => {
  const cache = temp();
  const out = temp();
  const victim = path.join(temp(), 'victim.txt');
  writeFileSync(victim, 'original');
  seedIndex(cache, 'magicui', [magicItem()]);
  mkdirSync(path.join(out, 'magicui', 'button'), { recursive: true });
  symlinkSync(victim, path.join(out, 'magicui', 'button', 'clayui.json'));
  raw(registryMock({ files: [{ path: 'a.tsx', content: 'export const a = 1' }] }), ['get', 'magicui/button', '--out', out, '--force'], cache);
  assert.equal(readFileSync(victim, 'utf8'), 'original');
});

test('R02 a provider response echoing a secret never reaches the output', () => {
  const cache = temp();
  seedIndex(cache, 'magicui', [magicItem()]);
  const echo = new URL('data:text/javascript,' + encodeURIComponent("globalThis.fetch = async (url, init) => new Response(init?.headers?.['x-goog-api-key'] ?? '{}')")).href;
  const result = raw(echo, ['search', 'button', '--source', 'magicui', '--json'], cache, { GEMINI_API_KEY: 'TEST_GEMINI_456' });
  assert.equal(result.status, 0);
  assert.ok(!result.stdout.includes('TEST_GEMINI_456') && !result.stderr.includes('TEST_GEMINI_456'));
});

test('R04 a damaged source entry does not stop healthy ones', () => {
  const cache = temp();
  writeFileSync(path.join(cache, 'index.json'), JSON.stringify({ sources: { magicui: { items: null }, shadcn: { items: [null, item('shadcn', 'dialog')], syncedAt: new Date().toISOString(), error: null } } }));
  const result = cli(['search', 'dialog', '--source', 'shadcn', '--json'], { cache });
  assert.equal(result.code, 0);
  assert.equal(JSON.parse(result.out).results[0].id, 'shadcn/dialog');
});

test('R07 invalid numeric flags exit 2', () => {
  for (const flags of [['--limit', 'abc'], ['--limit=-1'], ['--limit=1.5'], ['--limit=Infinity'], ['--details', 'abc'], ['--details=-1']]) {
    assert.equal(cli(['search', 'x', ...flags]).code, 2, flags.join(' '));
  }
});

test('R08 when every path is unsafe get exits 3 and creates nothing', () => {
  const cache = temp();
  const out = temp();
  seedIndex(cache, 'magicui', [magicItem()]);
  const result = raw(registryMock({ files: [{ path: '../../outside.tsx', content: 'x' }] }), ['get', 'magicui/button', '--out', out], cache);
  assert.equal(result.status, 3);
  assert.ok(!existsSync(path.join(out, 'magicui')));
});

test('R05 a detail with no code is no longer reported as free of gaps', () => {
  const cache = temp();
  seedIndex(cache, 'magicui', [magicItem()]);
  const shown = JSON.parse(raw(registryMock({ files: [] }), ['show', 'magicui/button', '--json'], cache).stdout);
  assert.ok(shown.gaps.includes('no-source-listing'));
});
