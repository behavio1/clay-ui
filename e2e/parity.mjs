// Runs the Node and Rust CLIs on the same inputs and compares them.
//   node e2e/parity.mjs          every source behind a local mock server
//   node e2e/parity.mjs --real   one real sync per CLI, then one real query per source
// Mock parity uses a debug binary: cargo build; release transport cannot be overridden.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, realpathSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const NODE_CLI = path.join(ROOT, 'bin/clayui.mjs');
const REAL = process.argv.includes('--real');
const RUST_CLI = path.join(ROOT, REAL ? 'target/release/clayui' : 'target/debug/clayui');
const work = realpathSync(mkdtempSync(path.join(os.tmpdir(), 'clayui-parity-')));

// --- Mock data for every source ---

const registryItem = (name, extra = {}) => ({ name, type: 'registry:ui', title: name.replace(/-/g, ' '), description: `The ${name} component`, dependencies: ['react'], registryDependencies: [], ...extra });
const NAMES = ['dialog', 'data-table', 'carousel', 'carousel-cards', 'toast', 'button', 'tabs', 'tooltip', 'drawer', 'marquee', 'aurora-background', 'loading-state'];
const itemBody = (name) => ({ description: `Detail for ${name}`, dependencies: ['react@19', '@radix-ui/react-dialog'], registryDependencies: ['button'], envVars: { A_KEY: 'x' }, files: [{ path: `registry/${name}.tsx`, target: `components/ui/${name}.tsx`, content: `export function ${name.replace(/-/g, '_')}() { return null }` }, { path: '../unsafe.tsx', content: 'x' }] });
const tree = (paths) => ({ tree: paths.map((file) => ({ path: file })) });
const tagPage = (tag, rows) => rows.map(([title, user, slug, variant]) => `- [${title}](https://21st.dev/@${user}/components/${slug}${variant ? `/${variant}` : ''}.md) description`).join('\n');
const llms = `# Base UI\n\n## Overview\n- [Intro](https://base-ui.com/react/overview.md): skip\n\n## Components\n${['dialog', 'menu', 'popover', 'select', 'tabs', 'slider'].map((name) => `- [${name}](https://base-ui.com/react/components/${name}.md): A ${name} primitive`).join('\n')}\n\n## Utilities\n- [X](https://base-ui.com/react/components/x.md): skip\n`;

// Sources added after the Node reference implementation. Parity covers only the sources both CLIs share,
// so these registries answer empty and their entries are left out of the comparison.
const RUST_ONLY = ['evilcharts', 'origin', 'kibo', 'reui', 'blocks-so', 'motion-primitives', 'smoothui', 'fancy', 'beui', 'uiarc', 'devl', 'cult', '8bitcn', 'shadcn-space', 'ui-layouts', 'spectrum', 'ruixen', 'watermelon', 'avara', 'hextaui', 'vengeance', 'componentry', 'asciicn'];
const RUST_ONLY_INDICES = ['evilcharts.com/r/registry.json', 'raw.githubusercontent.com/cosscom/ui/main/apps/origin/registry.json', 'www.kibo-ui.com/r/registry.json', 'reui.io/r/styles/base-nova/registry.json', 'blocks.so/r/registry.json', 'motion-primitives.com/c/registry.json', 'smoothui.dev/r/registry.json', 'fancycomponents.dev/r/registry.json', 'beui.dev/r/registry.json', 'uiarc.dev/r/registry.json', 'devl.dev/r/registry.json', 'www.cult-ui.com/r/registry.json', 'www.8bitcn.com/r/registry.json', 'shadcnspace.com/r/registry.json', 'raw.githubusercontent.com/shadcnspace/shadcnspace/main/registry.json', 'ui-layouts.com/r/registry.json', 'ui.spectrumhq.in/r/registry.json', 'ruixen.com/registry.json', 'registry.watermelon.sh/r/registry.json', 'avaraui.com/r/registry.json', 'hextaui.com/r/registry.json', 'vengenceui.com/r/registry.json', 'componentry.dev/r/registry.json', 'asciicn.dev/r/registry.json'];
const rustOnlyLine = (line) => RUST_ONLY.some((id) => new RegExp(`(^|[\\s"/])${id.replace('.', '\\.')}([\\s":/]|$)`).test(line));
const sharedOnly = (result) => ({ ...result, out: result.out.split('\n').filter((line) => !rustOnlyLine(line)).join('\n') });

// Rust-only sources are reported as skipped in searches; the Node reference does not know them.
function withoutRustOnlySkips(result) {
  let out = result.out;
  try {
    const body = JSON.parse(out);
    if (Array.isArray(body.skipped)) {
      body.skipped = body.skipped.filter((entry) => !RUST_ONLY.includes(entry.source));
      out = `${JSON.stringify(body, null, 2)}\n`;
    }
  } catch {}
  return { ...result, out, err: result.err.split('\n').filter((line) => !rustOnlyLine(line)).join('\n') };
}

function fixture(host, url) {
  if (RUST_ONLY_INDICES.includes(`${host}${url}`)) return { items: [] };
  const full = `${host}${url}`;
  if (host === 'ui.shadcn.com') {
    if (url === '/r/index.json') return [...NAMES.map((name) => registryItem(name, { meta: { links: { radix: { docs: `https://ui.shadcn.com/docs/${name}` } } } })), { name: 'hidden', type: 'registry:lib' }, { name: '../bad', type: 'registry:ui' }];
    const page = url.match(/^\/docs\/([^.]+)\.md$/);
    if (page) return `---\ntitle: ${page[1]}\ndescription: "Accessible ${page[1]} for React"\n---\n`;
    if (url.includes('/docs/')) return url.includes('tabs') ? 404 : '<html>';
    const item = url.match(/new-york-v4\/(.+)\.json$/);
    if (item) return item[1] === 'tooltip' ? 403 : item[1] === 'drawer' ? 404 : itemBody(item[1]);
  }
  if (host === 'magicui.design') {
    if (url === '/r/registry.json') return { items: NAMES.map((name, index) => registryItem(name, { type: index % 2 ? 'registry:component' : 'registry:block' })) };
    if (url.endsWith('.json')) return itemBody(url.split('/').pop().replace('.json', ''));
    return '<html>';
  }
  if (host === 'ui.aceternity.com') {
    if (url === '/registry/index.json') return NAMES.map((name) => registryItem(name));
    if (url === '/api/components') return { categories: { Effects: ['aurora-background', 'marquee'], Forms: ['button'] }, components: NAMES.slice(0, 8).map((name) => ({ name, title: `Ace ${name}`, description: `Aceternity ${name}`, documentationUrl: `https://ui.aceternity.com/components/${name}` })), blocks: [{ name: 'tabs', title: 'Ace Tabs Block' }] };
    if (url.includes('carousel.json')) return 403;
    if (url.endsWith('.json')) return itemBody(url.split('/').pop().replace('.json', ''));
    return '<html>';
  }
  if (host === 'raw.githubusercontent.com') {
    if (url === '/kokonut-labs/kokonutui/main/public/r/registry.json') return { items: NAMES.map((name) => registryItem(name)) };
    if (url.startsWith('/kokonut-labs/')) return itemBody(url.split('/').pop().replace('.json', ''));
    if (url.startsWith('/Codewithswappy/')) return `---\ntitle: 'Rare ${decodeURIComponent(url.split('/').pop())}'\ndescription: A rare component\n---\n`;
    if (url.startsWith('/radix-ui/website/')) return `---\nmetaDescription: Radix ${url.split('/').pop().replace('.mdx', '')} primitive\n---\n# x`;
    if (url.includes('/HEAD/')) return url.includes('broken') ? 500 : `export const file = ${JSON.stringify(url)};`;
  }
  if (full.startsWith('api.github.com/repos/')) {
    if (url.includes('kokonut-labs')) return tree(['content/docs/components/dialog.mdx', 'content/docs/blocks/data-table.mdx', 'content/other.mdx']);
    if (url.includes('Codewithswappy')) return tree(['content/docs/buttons/liquid-button.mdx', 'content/docs/ui/glass-card.mdx']);
    if (url.includes('radix-ui/website')) return tree(['data/primitives/docs/components/dialog.mdx', 'data/primitives/docs/components/tooltip.mdx', 'data/primitives/docs/components/tabs.mdx', 'data/primitives/docs/other/x.mdx']);
    const code = (dir) => tree([`${dir}/index.tsx`, `${dir}/style.css`, `${dir}/a.test.tsx`, `${dir}/__tests__/b.tsx`, `${dir}/sub/c.ts`, 'README.md']);
    if (url.includes('radix-ui/primitives')) return code('packages/react/dialog/src');
    if (url.includes('mui/base-ui')) return code('packages/react/src/dialog');
    if (url.includes('emilkowalski/sonner')) return code('src');
    if (url.includes('emilkowalski/vaul')) return 500;
    if (url.includes('dip/cmdk')) return code('cmdk/src');
    if (url.includes('barvian/number-flow')) return code('packages/react/src');
  }
  if (host === 'www.beautifului.dev') {
    if (url === '/r/registry.json') return { items: NAMES.map((name, index) => registryItem(name, { type: index % 2 ? 'registry:component' : 'registry:ui' })) };
    if (url.endsWith('.json')) return { ...itemBody(url.split('/').pop().replace('.json', '')), description: `Beautiful ${url}` };
    return '<html>';
  }
  if (host === 'rareui.in') {
    if (url === '/r/index.json') return { components: ['LiquidButton', 'GlassCard', 'NoDocsWidget', '../bad'] };
    if (url.endsWith('.json')) return itemBody('rare');
    return '<html>';
  }
  if (host === 'base-ui.com') return url === '/llms.txt' ? llms : url.endsWith('.md') ? `# Usage of ${url}` : '<html>';
  if (host === 'registry.npmjs.org') return url.includes('vaul') ? { license: { type: 'MIT' } } : { license: 'MIT' };
  if (host === 'unpkg.com') return url.includes('sonner') ? 500 : `# README of ${url}`;
  if (host === '21st.dev') {
    const tag = url.match(/\/community\/components\/s\/(.+)\.md$/);
    if (tag) {
      const pages = { carousel: [['Hero Carousel', 'alice', 'hero-carousel'], ['Card Slider', 'bob', 'card-slider', 'default']], slider: [['Card Slider', 'bob', 'card-slider'], ['Range Slider', 'carol', 'range']], dialog: [['Modal X', 'dan', 'modal-x']], modal: [['Modal Y', 'erin', 'modal-y']] };
      return pages[tag[1]] ? tagPage(tag[1], pages[tag[1]]) : 404;
    }
    if (url.startsWith('/api/v1/components/search')) return { results: [{ url: 'https://21st.dev/@alice/components/hero-carousel', name: 'Hero Carousel', description: 'Big carousel', tags: ['carousel', 'hero'] }, { url: 'https://21st.dev/@bob/components/card-slider' }, { url: 'nope' }] };
    if (url.startsWith('/api/v1/components/install/')) return { files: [{ path: 'component.tsx', content: 'export default 1' }] };
    if (url.endsWith('.md')) return `# Hero Component\n\n> Live description\n\n- License: ${url.includes('bob') ? 'unknown' : 'Apache-2.0'}\n- npm dependencies: react, motion,  clsx\n`;
    return '<html>';
  }
  if (['sonner.emilkowal.ski', 'vaul.emilkowal.ski', 'number-flow.barvian.me', 'www.radix-ui.com', 'kokonutui.com'].includes(host)) return url.includes('tabs') ? 410 : '<html>';
  return 404;
}

function embedding(text) {
  const vector = new Array(768).fill(0.11);
  for (const token of text.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean)) {
    const digest = createHash('sha256').update(token).digest();
    for (let index = 0; index < 8; index += 1) vector[digest.readUInt16LE(index * 2) % 768] += digest[16 + index] / 97.3;
  }
  return vector;
}

function startMock() {
  const server = http.createServer((request, response) => {
    const chunks = [];
    request.on('data', (chunk) => chunks.push(chunk));
    request.on('end', () => {
      const [, host, rest = ''] = request.url.match(/^\/([^/]+)(\/.*)?$/) ?? [];
      const send = (status, body) => { response.writeHead(status); response.end(typeof body === 'string' ? body : JSON.stringify(body)); };
      if (host === 'generativelanguage.googleapis.com') {
        const { requests } = JSON.parse(Buffer.concat(chunks).toString());
        return send(200, { embeddings: requests.map(({ content }) => ({ values: embedding(content.parts[0].text) })) });
      }
      const result = fixture(host, rest);
      // Tag pages finish in tag order so that Node's completion-order merge is repeatable.
      const delay = rest.includes('/s/slider') || rest.includes('/s/modal') ? 40 : 0;
      setTimeout(() => (typeof result === 'number' ? send(result, 'error') : send(200, result)), delay);
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

function writePreload(base) {
  const file = path.join(work, 'preload.mjs');
  writeFileSync(file, `const real = globalThis.fetch;
globalThis.fetch = async (url, init) => {
  const target = new URL(url);
  const response = await real(${JSON.stringify(base)} + '/' + target.host + target.pathname + target.search, init);
  Object.defineProperty(response, 'url', { value: url });
  return response;
};
`);
  return file;
}

// --- Running both CLIs ---

function runAsync(command, args, env, cwd) {
  return new Promise((resolve) => {
    const child = spawn(command, args, { env, cwd });
    let out = '';
    let err = '';
    child.stdout.on('data', (data) => { out += data; });
    child.stderr.on('data', (data) => { err += data; });
    child.on('close', (code) => resolve({ code, out, err }));
  });
}

function makeRunner(kind, base, preload) {
  const folder = path.join(work, kind);
  const [cache, home, cwd] = ['cache', 'home', 'cwd'].map((name) => path.join(folder, name));
  for (const directory of [cache, home, cwd]) mkdirSync(directory, { recursive: true });
  const env = { PATH: process.env.PATH, HOME: home, CLAYUI_CACHE_DIR: cache };
  if (process.env.CLAYUI_GEMINI_ENV_FILE && REAL) env.CLAYUI_GEMINI_ENV_FILE = process.env.CLAYUI_GEMINI_ENV_FILE;
  if (base) env.CLAYUI_TEST_BASE_URL = base;
  const command = kind === 'node' ? process.execPath : RUST_CLI;
  const prefix = kind === 'node' ? (preload ? ['--import', preload, NODE_CLI] : [NODE_CLI]) : [];
  return { kind, cache, cwd, home, run: (args, extra = {}) => runAsync(command, [...prefix, ...args], { ...env, ...extra }, cwd) };
}

// Text that legitimately differs: the English-only rule and per-run paths and times.
function normalizeText(text, runner) {
  return text
    .split(runner.cwd).join('<cwd>')
    .replace(/\d{4}-\d\d-\d\dT[\d:.]+Z/g, '<time>')
    .replaceAll('Uwaga:', 'Warning:')
    .replaceAll('UWAGA:     ', 'WARNING:   ')
    .replaceAll('rejestr:   ', 'registry:  ')
    .replaceAll('plik:      ', 'file:      ')
    .replaceAll('projekt:   ', 'project:   ')
    .replaceAll('Niepoprawny package.json w', 'Invalid package.json in');
}

const results = { passed: 0, failed: [] };

function record(label, node, rust, compareErr) {
  const same = node.code === rust.code && normalizeText(node.out, nodeRunner) === normalizeText(rust.out, rustRunner) && (!compareErr || normalizeText(node.err, nodeRunner) === normalizeText(rust.err, rustRunner));
  if (same) results.passed += 1;
  else results.failed.push({ label, node: { code: node.code, out: node.out.slice(0, 1500), err: node.err.slice(0, 500) }, rust: { code: rust.code, out: rust.out.slice(0, 1500), err: rust.err.slice(0, 500) } });
  return same;
}

function listTree(directory, prefix = '') {
  const found = [];
  for (const name of readdirSync(directory).sort()) {
    const full = path.join(directory, name);
    if (statSync(full).isDirectory()) found.push(...listTree(full, `${prefix}${name}/`));
    else found.push(`${prefix}${name}`);
  }
  return found;
}

let nodeRunner;
let rustRunner;

async function compare(label, args, { env, compareErr = false } = {}) {
  const [node, rust] = await Promise.all([nodeRunner.run(args, env), rustRunner.run(args, env)]);
  return record(`${label}: ${args.join(' ')}`, node, withoutRustOnlySkips(rust), compareErr);
}

function comparableIndex(runner) {
  const index = JSON.parse(readFileSync(path.join(runner.cache, 'index.json'), 'utf8'));
  for (const id of RUST_ONLY) delete index.sources[id];
  for (const entry of Object.values(index.sources)) entry.syncedAt = entry.syncedAt ? '<time>' : null;
  return JSON.stringify(index);
}

async function mockParity() {
  const server = await startMock();
  try {
  const base = `http://127.0.0.1:${server.address().port}`;
  const preload = writePreload(base);
  nodeRunner = makeRunner('node', base, preload);
  rustRunner = makeRunner('rust', base, null);

  // Sync log lines appear in completion order in both CLIs, so they are compared sorted.
  const [nodeSync, rustSync] = await Promise.all([nodeRunner.run(['sync']), rustRunner.run(['sync'])]);
  const sorted = (result) => ({ ...result, out: result.out.split('\n').sort().join('\n') });
  record('sync (no key)', sorted(nodeSync), sorted(sharedOnly(rustSync)));
  assert.equal(comparableIndex(nodeRunner), comparableIndex(rustRunner), 'index.json differs after sync');
  results.passed += 1;
  for (const [label, args] of [['sources', ['sources', '--json']], ['sources text', ['sources']]]) {
    const [node, rust] = await Promise.all([nodeRunner.run(args), rustRunner.run(args)]);
    const shared = args.includes('--json') ? { ...rust, out: `${JSON.stringify(JSON.parse(rust.out).filter((source) => !RUST_ONLY.includes(source.id)), null, 2)}\n` } : sharedOnly(rust);
    record(`${label}: ${args.join(' ')}`, node, shared);
  }

  const queries = ['dialog', 'carousel cards', 'data table with filters', 'toast notification', 'modal', 'loading state', 'aurora background', 'liquid button', 'primitive', 'animated number counter', 'command palette', 'bottom sheet', 'the and of', 'qqqzzz', 'Żółć dialog'];
  const sources = ['', 'shadcn', 'magicui', 'aceternity', 'kokonut', 'beautifului', '21st', 'rareui', 'base-ui', 'radix', 'sonner', 'vaul', 'cmdk', 'number-flow'];
  for (const [mode, env] of [['bm25', {}], ['semantic', { GEMINI_API_KEY: 'mock-gemini-key-value' }], ['with-21st-key', { GEMINI_API_KEY: 'mock-gemini-key-value', API_KEY_21ST: 'mock-21st-key-value' }]]) {
    for (const query of queries) {
      for (const source of sources) await compare(mode, ['search', query, ...(source ? ['--source', source] : []), '--limit', '50', '--json'], { env });
    }
    await compare(mode, ['search', 'dialog', '--details', '3', '--json'], { env });
    await compare(mode, ['search', 'dialog', '--details', '2', '--code', '--json'], { env });
    await compare(mode, ['search', 'carousel', '--source', 'shadcn', '--details', '2', '--code'], { env, compareErr: true });
    await compare(mode, ['search', 'dialog', '--limit', '3'], { env, compareErr: true });
    assert.deepEqual(readFileSync(path.join(rustRunner.cache, 'index.json')).length > 0, true);
  }
  const nodeVectors = JSON.parse(readFileSync(path.join(nodeRunner.cache, 'embeddings.json'), 'utf8'));
  const rustVectors = JSON.parse(readFileSync(path.join(rustRunner.cache, 'embeddings.json'), 'utf8'));
  assert.deepEqual(Object.keys(rustVectors).sort(), Object.keys(nodeVectors).sort(), 'vector cache keys differ');
  assert.deepEqual(rustVectors, nodeVectors, 'vector cache values differ');
  results.passed += 2;

  const ids = ['shadcn/dialog', 'shadcn/tooltip', 'shadcn/drawer', 'magicui/button', 'aceternity/carousel', 'aceternity/marquee', 'kokonut/dialog', 'beautifului/tabs', 'rareui/LiquidButton', 'base-ui/dialog', 'radix/dialog', 'sonner/sonner', 'vaul/vaul', 'cmdk/cmdk', 'number-flow/number-flow', '21st/alice/hero-carousel', '21st/bob/card-slider'];
  writeFileSync(path.join(nodeRunner.cwd, 'package.json'), '{"dependencies":{"react":"19"},"devDependencies":{"tailwindcss":"4"}}');
  writeFileSync(path.join(rustRunner.cwd, 'package.json'), '{"dependencies":{"react":"19"},"devDependencies":{"tailwindcss":"4"}}');
  for (const env of [{}, { API_KEY_21ST: 'mock-21st-key-value' }]) {
    for (const id of ids) {
      await compare('show', ['show', id, '--json'], { env });
      await compare('show code', ['show', id, '--code', '--json', '--project', '.'], { env });
      await compare('show text', ['show', id, '--code', '--project', '.'], { env, compareErr: true });
      const destination = env.API_KEY_21ST ? 'out-key' : 'out';
      await compare('get', ['get', id, '--out', destination, '--json'], { env, compareErr: true });
      await compare('get again', ['get', id, '--out', destination], { env });
      await compare('get force', ['get', id, '--out', destination, '--force'], { env, compareErr: true });
    }
    const destination = env.API_KEY_21ST ? 'out-key' : 'out';
    const [nodeOut, rustOut] = [nodeRunner, rustRunner].map((runner) => path.join(runner.cwd, destination));
    assert.deepEqual(listTree(rustOut), listTree(nodeOut), 'downloaded file lists differ');
    for (const file of listTree(nodeOut).filter((name) => !name.endsWith('clayui.json'))) assert.equal(readFileSync(path.join(rustOut, file), 'utf8'), readFileSync(path.join(nodeOut, file), 'utf8'), file);
    for (const file of listTree(nodeOut).filter((name) => name.endsWith('clayui.json'))) {
      const clean = (text) => text.replace(/"fetchedAt": "[^"]+"/, '');
      assert.equal(clean(readFileSync(path.join(rustOut, file), 'utf8')), clean(readFileSync(path.join(nodeOut, file), 'utf8')), file);
    }
    results.passed += 1;
  }

  const usage = [['frobnicate'], ['add', 'x'], ['search'], ['search', 'x', '--limit', 'abc'], ['search', 'x', '--limit=-1'], ['search', 'x', '--limit=1.5'], ['search', 'x', '--limit=Infinity'], ['search', 'x', '--limit', '-1'], ['search', 'x', '--limit'], ['search', 'x', '--details=-1'], ['search', 'x', '--limit=0x2', '--json'], ['search', 'x', '--limit=1e1', '--json'], ['search', 'x', '--source', 'nope'], ['search', 'x', '--bogus'], ['search', 'x', '-z'], ['search', 'x', '--json=1'], ['search', '--', '--json'], ['show'], ['show', 'nope'], ['show', 'magicui/../x'], ['show', 'magicui/-rf'], ['show', 'zzz/x'], ['show', 'magicui/not-there'], ['show', 'magicui/button', '--project', 'missing'], ['get', 'magicui/not-there'], ['sync', '--source', 'nope'], ['init', '--target', 'nope'], ['--help'], ['-h'], []];
  for (const args of usage) await compare('contract', args, { compareErr: args[0] === undefined || args[0] === '--help' });
  } finally { server.close(); }
}

async function realParity() {
  if (!process.env.CLAYUI_GEMINI_ENV_FILE) console.log('Warning: CLAYUI_GEMINI_ENV_FILE not set, semantic search not compared');
  nodeRunner = makeRunner('node');
  rustRunner = makeRunner('rust');
  console.log('Syncing with Node...');
  const node = await nodeRunner.run(['sync']);
  console.log(node.out.split('\n').slice(0, 14).join('\n'), node.code);
  console.log('Syncing with Rust...');
  const rust = await rustRunner.run(['sync']);
  console.log(rust.out.split('\n').slice(0, 14).join('\n'), rust.code);
  const load = (runner) => JSON.parse(readFileSync(path.join(runner.cache, 'index.json'), 'utf8')).sources;
  const [nodeIndex, rustIndex] = [load(nodeRunner), load(rustRunner)];
  const report = { indexDifferences: [], order: [] };
  for (const [id, entry] of Object.entries(nodeIndex)) {
    const left = entry.items.map((item) => item.id);
    const right = (rustIndex[id]?.items ?? []).map((item) => item.id);
    let identicalFields = 0;
    const differingFields = new Set();
    const others = new Map((rustIndex[id]?.items ?? []).map((item) => [item.id, item]));
    for (const item of entry.items) {
      const other = others.get(item.id);
      if (other && JSON.stringify(other) === JSON.stringify(item)) identicalFields += 1;
      else if (other) for (const key of Object.keys(item)) if (JSON.stringify(item[key]) !== JSON.stringify(other[key])) differingFields.add(key);
    }
    report.indexDifferences.push({ source: id, node: left.length, rust: right.length, sameOrder: JSON.stringify(left) === JSON.stringify(right), itemsByteIdentical: identicalFields, differingFields: [...differingFields].join(',') || '-', nodeError: entry.error, rustError: rustIndex[id]?.error ?? null });
  }
  console.table(report.indexDifferences);
  const CASES = [['shadcn', 'dialog'], ['magicui', 'marquee scrolling'], ['aceternity', 'aurora background'], ['kokonut', 'carousel cards'], ['beautifului', 'loading state'], ['21st', 'carousel'], ['rareui', 'liquid button'], ['base-ui', 'dialog'], ['radix', 'tooltip'], ['sonner', 'toast'], ['vaul', 'drawer'], ['cmdk', 'command menu'], ['number-flow', 'animated number'], [null, 'sortable data table with filters'], [null, 'toast notification'], [null, 'pricing section with animated counter']];
  const noKey = { GEMINI_API_KEY: '', CLAYUI_GEMINI_ENV_FILE: '' };
  const ids = (result) => { try { return JSON.parse(result.out).results.map((item) => item.id); } catch { return `unparsable (exit ${result.code})`; } };
  for (const [mode, env] of [['semantic', {}], ['bm25', noKey]]) {
    for (const [source, query] of CASES) {
      const args = ['search', query, ...(source ? ['--source', source] : []), '--limit', '10', '--json'];
      const [left, right] = [await nodeRunner.run(args, env), await rustRunner.run(args, env)];
      const [a, b] = [ids(left), ids(right)];
      const same = JSON.stringify(a) === JSON.stringify(b);
      report.order.push({ mode, source: source ?? 'all', query, same, count: Array.isArray(a) ? a.length : a, first: Array.isArray(a) ? a[0] : '' });
      if (!same) console.log('DIFFERENT', mode, source, query, '\n node:', a, '\n rust:', b);
    }
  }
  console.table(report.order);
  const details = [];
  for (const id of ['shadcn/dialog', 'magicui/marquee', 'aceternity/aurora-background', 'kokonut/carousel-cards', 'beautifului/loading-state', 'rareui/LiquidButton', 'base-ui/dialog', 'radix/tooltip', 'sonner/sonner', 'vaul/vaul', 'cmdk/cmdk', 'number-flow/number-flow']) {
    const args = ['get', id, '--out', 'out', '--json'];
    const [left, right] = [await nodeRunner.run(args), await rustRunner.run(args)];
    const files = (result) => { try { return JSON.parse(result.out).files; } catch { return null; } };
    const showArgs = ['show', id, '--json'];
    const [shownLeft, shownRight] = [await nodeRunner.run(showArgs), await rustRunner.run(showArgs)];
    const preview = (result) => { try { return JSON.parse(result.out).previewStatus; } catch { return `exit ${result.code}`; } };
    details.push({ id, exit: `${left.code}/${right.code}`, sameFiles: JSON.stringify(files(left)) === JSON.stringify(files(right)), files: files(left)?.length, preview: `${preview(shownLeft)}/${preview(shownRight)}` });
  }
  console.table(details);
  const orderFailures = report.order.filter((row) => !row.same);
  const indexFailures = report.indexDifferences.filter((row) => !row.sameOrder || row.node !== row.rust || row.differingFields !== '-');
  const detailFailures = details.filter((row) => row.exit.split('/')[0] !== row.exit.split('/')[1] || !row.sameFiles);
  console.log(`Sources with differing index: ${indexFailures.length}/${report.indexDifferences.length}. Queries with differing order: ${orderFailures.length}/${report.order.length}. Downloads differing: ${detailFailures.length}/${details.length}.`);
  process.exitCode = orderFailures.length || indexFailures.length || detailFailures.length ? 1 : 0;
}

const main = REAL ? realParity : mockParity;
main().then(() => {
  if (!REAL) {
    console.log(`Compared cases passed: ${results.passed}, failed: ${results.failed.length}`);
    for (const failure of results.failed.slice(0, 12)) console.log(JSON.stringify(failure, null, 1));
    process.exitCode = results.failed.length ? 1 : 0;
  }
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
}).finally(() => rmSync(work, { recursive: true, force: true }));
