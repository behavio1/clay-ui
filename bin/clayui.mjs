#!/usr/bin/env node
// Development-only Node reference; the published launcher runs the Rust CLI.
import { closeSync, constants, existsSync, lstatSync, mkdirSync, openSync, readFileSync, renameSync, writeFileSync, writeSync } from 'node:fs';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { missingLocalImports } from '../src/imports.mjs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { inspectProject, missingDependencies, safeRelative, UsageError } from '../src/project.mjs';
import { VectorStore, cosine, embedQuery, readKey } from '../src/embeddings.mjs';
import { expandQuery, rank, rankSemantic } from '../src/search.mjs';
import { SOURCES, previewStatus, sourceById, searchLicense } from '../src/sources.mjs';

const STALE_MS = 7 * 24 * 60 * 60 * 1000;
const EXIT_NO_CODE = 3;

const HELP = `Clay UI — find proven UI components, compare variants and download their code as examples

  clayui init [--global] [--target agents|claude|all] [--no-sync]
  clayui search "<description>" [--source s] [--limit n] [--details n] [--available-only] [--code] [--json]
  clayui show <id> [--project p] [--code] [--json]
  clayui get <id> [--out folder] [--force] [--json]
  clayui sync [--source s] [--json]
  clayui sources [--json]

Nothing is ever installed into your project: get only downloads example code to a local folder.\nSet GEMINI_API_KEY for semantic search. Code from 21st.dev needs API_KEY_21ST.`;

const cacheDir = () => process.env.CLAYUI_CACHE_DIR ?? path.join(os.homedir(), '.cache', 'clayui');
const indexPath = () => path.join(cacheDir(), 'index.json');

const validCandidate = (item) => item && typeof item === 'object' && typeof item.id === 'string' && typeof item.name === 'string' && typeof item.source === 'string' && Array.isArray(item.gaps);

// A damaged source entry is dropped so that healthy sources keep working.
function readIndex() {
  let index;
  try {
    index = JSON.parse(readFileSync(indexPath(), 'utf8'));
  } catch {
    return null;
  }
  if (!index?.sources || typeof index.sources !== 'object') return null;
  for (const [id, entry] of Object.entries(index.sources)) {
    if (!Array.isArray(entry?.items)) delete index.sources[id];
    else entry.items = entry.items.filter(validCandidate);
  }
  return index;
}

function parseCount(value, name, fallback, minimum) {
  if (value === undefined) return fallback;
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < minimum) throw new UsageError(`${name} must be an integer of at least ${minimum}`);
  return number;
}

const hasCode = (detail) => detail.files?.some((file) => typeof file.content === 'string' && file.content);

// Gaps reflect what the detail read actually found, not what the index assumed.
function withObservedGaps(detail, status) {
  const gaps = new Set(detail.gaps);
  if (!hasCode(detail) && !gaps.has('pro-only') && !gaps.has('needs-key')) gaps.add('no-source-listing');
  if (status === 'broken') gaps.add('preview-broken');
  const missing = missingLocalImports(detail.files ?? []);
  if (missing.length) gaps.add('local-imports-missing');
  return { ...detail, missingLocalImports: missing, gaps: [...gaps], previewStatus: status === 'unverified' && detail.fromCache ? 'unknown' : status };
}

function writeIndex(index) {
  mkdirSync(cacheDir(), { recursive: true });
  const temporary = `${indexPath()}.${process.pid}.tmp`;
  writeFileSync(temporary, JSON.stringify(index));
  renameSync(temporary, indexPath());
}

function selectSources(id) {
  if (!id) return SOURCES;
  const source = sourceById(id);
  if (!source) throw new UsageError(`Unknown source: ${id}`);
  return [source];
}

async function sync(sources, log) {
  const index = readIndex() ?? { sources: {} };
  await Promise.all(sources.filter((source) => !source.live).map(async (source) => {
    const entry = (index.sources[source.id] ??= { items: [], syncedAt: null, error: null });
    try {
      entry.items = await source.load();
      entry.syncedAt = new Date().toISOString();
      entry.error = null;
      log(`${source.id}: ${entry.items.length}`);
    } catch (error) {
      entry.error = error.message;
      log(`${source.id}: ERROR ${error.message}`);
    }
  }));
  writeIndex(index);
  const key = readKey();
  if (key) {
    const store = new VectorStore(cacheDir());
    try {
      await store.ensure(Object.values(index.sources).flatMap((entry) => entry.items), key);
      log('embeddings: ready');
    } catch (error) {
      log(`embeddings: ERROR ${error.message}`);
    } finally {
      store.save();
    }
  } else {
    log('embeddings: no key, using keyword search');
  }
  return index;
}

const silent = () => {};
const stderr = (line) => console.error(line);

async function cmdSync({ values }) {
  await sync(selectSources(values.source), console.log);
}

function cmdSources({ values }) {
  const index = readIndex() ?? { sources: {} };
  const rows = SOURCES.map((source) => {
    const entry = index.sources[source.id];
    return { id: source.id, title: source.title, access: source.access, items: source.live ? null : entry?.items.length ?? 0, syncedAt: entry?.syncedAt ?? null, error: entry?.error ?? null };
  });
  if (values.json) return console.log(JSON.stringify(rows, null, 2));
  for (const row of rows) {
    const count = row.items === null ? 'live' : `${row.items} items`;
    console.log(`${row.id.padEnd(13)} ${count.padEnd(10)} ${row.access.padEnd(24)} ${row.syncedAt?.slice(0, 10) ?? '-'}${row.error ? `  ERROR: ${row.error}` : ''}`);
  }
}

async function cmdSearch({ values, positionals }) {
  const limit = parseCount(values.limit, '--limit', 20, 1);
  const detailed = parseCount(values.details, '--details', 0, 0);
  const query = positionals.join(' ').trim();
  if (!query) throw new UsageError('Describe the component you need');
  const sources = selectSources(values.source);
  const index = readIndex() ?? await sync(SOURCES, values.json ? silent : stderr);
  const warnings = [];
  const skipped = [];
  const pool = [];
  for (const source of sources.filter((item) => !item.live)) {
    const entry = index.sources[source.id];
    if (entry?.items.length) pool.push(...entry.items);
    if (entry?.error) skipped.push({ source: source.id, reason: entry.error });
    else if (!entry?.items.length) skipped.push({ source: source.id, reason: 'not indexed yet, run sync' });
    if (entry?.syncedAt && Date.now() - Date.parse(entry.syncedAt) > STALE_MS) warnings.push(`The ${source.id} index is over 7 days old, run sync`);
  }
  const live = sources.find((source) => source.live);
  const groups = expandQuery(query);
  if (live && groups.length) {
    try {
      const result = await live.searchLive(groups);
      pool.push(...result.items);
      warnings.push(...result.warnings);
    } catch (error) {
      skipped.push({ source: live.id, reason: error.message });
    }
  }
  const availableOnly = values['available-only'];
  const options = { limit: availableOnly ? Infinity : limit, perSource: availableOnly || values.source ? Infinity : 5 };
  let results = null;
  const key = readKey();
  if (key && groups.length) {
    const store = new VectorStore(cacheDir());
    try {
      const [vectors, queryVector] = await Promise.all([store.ensure(pool, key), embedQuery(query, key)]);
      const similarity = new Map(pool.map((item) => [item.id, cosine(queryVector, vectors.get(item.id))]));
      results = rankSemantic(pool, similarity, options);
    } catch (error) {
      warnings.push(`Semantic search unavailable (${error.message}), used keyword search`);
    } finally {
      store.save();
    }
  } else if (!key) {
    warnings.push('No Gemini key, used keyword search. Set GEMINI_API_KEY for much better results');
  }
  results ??= rank(pool, query, options);
  if (availableOnly) {
    const counts = new Map();
    results = results.filter(item => {
      if (item.gaps.some(g => ['needs-key', 'pro-only', 'no-source-listing'].includes(g))) return false;
      const count = counts.get(item.source) ?? 0;
      if (!values.source && count >= 5) return false;
      counts.set(item.source, count + 1);
      return true;
    }).slice(0, limit);
  }
  results = await Promise.all(results.map(searchLicense));
  if (detailed > 0) {
    results = await Promise.all(results.map(async (item, index) => {
      if (index >= detailed) return item;
      try {
        const detail = await cachedDetail(sourceById(item.source), item);
        const observed = withObservedGaps(detail, await previewStatus(detail.previewUrl));
        return { ...(values.code ? observed : withoutCode(observed)), score: item.score };
      } catch (error) {
        return { ...item, detailError: error.message };
      }
    }));
  }
  if (availableOnly) results = results.filter(item => !item.detailError && !item.gaps.some(g => ['needs-key', 'pro-only', 'no-source-listing'].includes(g)));
  if (values.json) return console.log(JSON.stringify({ query, results, skipped, warnings }, null, 2));
  if (!results.length) console.log('No matches.');
  for (const item of results) {
    const flag = item.gaps.length ? `gaps: ${item.gaps.join(', ')}` : 'complete';
    console.log(`${item.id}  [${flag}]\n    ${item.title}${item.description ? ` — ${item.description.slice(0, 100)}` : ''}\n    preview: ${item.previewUrl ?? 'none'}${item.previewStatus ? ` (${item.previewStatus})` : ''}`);
    if (item.detailError) console.log(`    details error: ${item.detailError}`);
    if (item.files) printFiles(item, values.code);
  }
  for (const warning of warnings) console.error(`Uwaga: ${warning}`);
  for (const { source, reason } of skipped) console.error(`Skipped ${source}: ${reason}`);
}

function parseId(id) {
  const match = /^([a-z0-9-]+|21st)\/([A-Za-z0-9._@/-]+)$/.exec(id ?? '');
  if (!match) throw new UsageError(`Invalid id: ${id ?? '(none)'}`);
  const [, sourceId, name] = match;
  const source = sourceById(sourceId);
  if (!source) throw new UsageError(`Unknown source: ${sourceId}`);
  if (name.split('/').some((part) => part === '..' || part === '.' || part.startsWith('-'))) throw new UsageError(`Invalid id: ${id}`);
  return { source, name };
}

async function loadDetail(id) {
  const { source, name } = parseId(id);
  let item;
  if (source.live) {
    item = source.fromId(name);
  } else {
    const index = readIndex() ?? await sync(SOURCES, stderr);
    item = index.sources[source.id]?.items.find((entry) => entry.name === name);
  }
  if (!item) throw new UsageError(`Not found: ${id}. Run clayui sync or search`);
  const detail = await cachedDetail(source, item);
  return detail.license?.toLowerCase() === 'mit' ? { ...detail, license: 'MIT' } : detail;
}

async function cachedDetail(source, item) {
  const key = createHash('sha256').update(JSON.stringify(item)).digest('hex');
  const file = path.join(cacheDir(), 'reference-components', key + '.json');
  let retained;
  try { retained = JSON.parse(readFileSync(file, 'utf8')); } catch {}
  const age = retained ? Date.now() - Date.parse(retained.fetchedAt) : Infinity;
  if (age < 7 * 86400000) return { ...retained, fromCache: true };
  try {
    const detail = { ...await source.detail(item), fetchedAt: new Date(Math.floor(Date.now() / 1000) * 1000).toISOString(), fromCache: false };
    if (hasCode(detail)) { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, JSON.stringify(detail)); return detail; }
    if (!retained || age > 30 * 86400000) return detail;
  } catch (error) { if (!retained || age > 30 * 86400000) throw error; }
  const days = Math.floor(age / 86400000);
  return { ...retained, fromCache: true, cacheAgeDays: days, cacheWarning: `Warning: upstream unavailable; using cached component aged ${days} days. Retry when the source is available.` };
}

function cleanDescription(detail) {
  if (typeof detail.description !== 'string') return detail;
  const text = detail.description;
  const at = text.search(/npx |pnpm dlx |pnpm add |npm install |yarn add |bunx |bun add /);
  if (at < 0) return detail;
  let cleaned = text.slice(0, at);
  if (at >= 0) {
    const lower = cleaned.toLowerCase();
    const labels = ['install with', 'install via', 'install:', 'installation:', '```'].map(l => lower.lastIndexOf(l)).filter(i => i >= 0);
    if (labels.length) cleaned = cleaned.slice(0, Math.min(...labels));
  }
  return { ...detail, description: cleaned.replace(/[\s`:-]+$/, '').trim() };
}

function withoutCode(detail) {
  const { usage, ...rest } = detail;
  return { ...rest, files: rest.files?.map(({ content, ...file }) => file) };
}

function printFiles(detail, code) {
  console.log(`deps:      ${detail.dependencies?.join(', ') || '-'}`);
  console.log(`rejestr:   ${detail.registryDependencies?.join(', ') || '-'}`);
  for (const warning of detail.sourceWarnings ?? []) console.log(`UWAGA:     ${warning}`);
  for (const file of detail.files ?? []) {
    console.log(`plik:      ${file.target || file.path}`);
    if (code && file.content) console.log(`\n${file.content}\n`);
  }
  if (code && detail.usage) console.log(`\n--- usage ---\n${detail.usage}\n--- end ---\n`);
}

async function cmdShow({ values, positionals }) {
  const project = values.project ? inspectProject(values.project) : null;
  const detail = await loadDetail(positionals[0]);
  const observed = cleanDescription(withObservedGaps(detail, await previewStatus(detail.previewUrl)));
  const output = { ...(values.code ? observed : withoutCode(observed)), project: project && { react: project.react, tailwind: project.tailwind, missingDependencies: missingDependencies(detail, project) } };
  if (values.json) return console.log(JSON.stringify(output, null, 2));
  console.log(`${output.id}\n${output.title}${output.description ? ` — ${output.description}` : ''}`);
  console.log(`preview: ${output.previewUrl ?? 'none'} (${output.previewStatus})`);
  console.log(`license:   ${output.license ?? 'unknown'}${output.licenseAssumed ? ' (assumed)' : ''}`);
  console.log(`gaps:      ${output.gaps.join(', ') || 'none'}`);
  printFiles(output, values.code);
  if (output.project) console.log(`projekt:   ${JSON.stringify(output.project)}`);
}

// Creates each folder below base, refusing symlinks so a write can never leave the example folder.
function ensureRealFolder(base, parts) {
  mkdirSync(base, { recursive: true });
  let current = base;
  for (const part of parts) {
    current = path.join(current, part);
    if (existsSync(current) || isLink(current)) {
      if (lstatSync(current).isSymbolicLink()) throw new Error(`Refusing to write through a symbolic link: ${current}`);
    } else {
      mkdirSync(current);
    }
  }
  return current;
}

function isLink(file) {
  try {
    return lstatSync(file).isSymbolicLink();
  } catch {
    return false;
  }
}

function writeNoFollow(file, content) {
  const descriptor = openSync(file, constants.O_WRONLY | constants.O_CREAT | constants.O_TRUNC | constants.O_NOFOLLOW, 0o644);
  try {
    writeSync(descriptor, content);
  } finally {
    closeSync(descriptor);
  }
}

// Downloads example source only. Content is untrusted: it is written as data and never executed.
async function cmdGet({ values, positionals }) {
  const loaded = await loadDetail(positionals[0]);
  const detail = cleanDescription(withObservedGaps(loaded, await previewStatus(loaded.previewUrl)));
  const base = path.resolve(values.out ?? 'clayui-examples');
  const parts = [detail.source, ...detail.name.split('/')];
  const destination = path.join(base, ...parts);
  if ((existsSync(destination) || isLink(destination)) && !values.force) throw new UsageError(`Folder already exists: ${destination}. Use --force`);
  const skipped = [...(detail.skipped ?? [])];
  const unsafe = [];
  const files = [];
  for (const file of detail.files ?? []) {
    if (typeof file.content !== 'string' || !file.content.length) { skipped.push(file.target || file.path); continue; }
    const relative = safeRelative(file.target || file.path);
    if (relative) files.push({ relative, content: file.content });
    else { skipped.push(file.target || file.path); unsafe.push(file.target || file.path); }
  }
  for (const warning of detail.sourceWarnings ?? []) console.error(`Uwaga: ${warning}`);
  for (const file of unsafe) console.error(`Skipped unsafe path: ${file}`);
  if (!files.length) {
    const reason = detail.gaps.includes('needs-key') ? ': requires API_KEY_21ST' : detail.gaps.includes('pro-only') ? ': paid item, requires a license from its author' : '';
    console.error(`Code for ${detail.id} is unavailable${reason}; no source files were downloaded. Usage documentation alone is not source code. Preview: ${detail.previewUrl ?? 'none'}`);
    process.exitCode = EXIT_NO_CODE;
    return;
  }
  ensureRealFolder(base, parts);
  const written = [];
  const fileSha256 = {};
  for (const { relative, content } of files) {
    const segments = relative.split('/');
    writeNoFollow(path.join(ensureRealFolder(destination, segments.slice(0, -1)), segments.at(-1)), content);
    fileSha256[relative] = createHash('sha256').update(content).digest('hex');
    written.push(relative);
  }
  if (detail.usage) {
    writeNoFollow(path.join(destination, 'USAGE.md'), detail.usage);
    fileSha256['USAGE.md'] = createHash('sha256').update(detail.usage).digest('hex');
    written.push('USAGE.md');
  }
  const meta = { id: detail.id, title: detail.title, description: detail.description, previewUrl: detail.previewUrl, previewStatus: detail.previewStatus, sourceUrl: detail.sourceUrl, sourceRef: detail.sourceRef, version: detail.version, missingLocalImports: detail.missingLocalImports, license: detail.license, licenseAssumed: Boolean(detail.licenseAssumed), gaps: detail.gaps, sourceWarnings: detail.sourceWarnings ?? [], dependencies: detail.dependencies, registryDependencies: detail.registryDependencies ?? [], envVars: detail.envVars ?? [], skipped, files: written, fileSha256, fetchedAt: detail.fetchedAt, fromCache: detail.fromCache };
  writeNoFollow(path.join(destination, 'clayui.json'), JSON.stringify(meta, null, 2));
  if (values.json) return console.log(JSON.stringify({ destination, ...meta }, null, 2));
  console.log(`${detail.id} → ${destination}\nlicense: ${detail.license ?? 'unknown'}${detail.licenseAssumed ? ' (assumed)' : ''}\nfiles:    ${written.join(', ')}`);
}

const SKILL_FOLDERS = { agents: ['.agents', 'skills', 'clay-ui'], claude: ['.claude', 'skills', 'clay-ui'] };

// Copies the bundled agent skill so a coding agent can use Clay UI straight away.
async function cmdInit({ values }) {
  const target = values.target ?? 'all';
  const names = target === 'all' ? Object.keys(SKILL_FOLDERS) : [target];
  if (names.some((name) => !SKILL_FOLDERS[name])) throw new UsageError('--target must be agents, claude or all');
  const base = values.global ? os.homedir() : process.cwd();
  const skill = readFileSync(new URL('../skills/clay-ui/SKILL.md', import.meta.url), 'utf8');
  for (const name of names) {
    const folder = ensureRealFolder(base, SKILL_FOLDERS[name]);
    writeNoFollow(path.join(folder, 'SKILL.md'), skill);
    console.log(`Skill installed: ${folder}`);
  }
  if (!values['no-sync']) await cmdSync({ values: {} });
  console.log(`\nDone. Ask your coding agent for a component, for example: "find me a nice data table with Clay UI".${readKey() ? '' : '\nTip: set GEMINI_API_KEY or CLAYUI_GEMINI_ENV_FILE for semantic search. Without it Clay UI falls back to keyword search.'}`);
}

const COMMANDS = { init: cmdInit, search: cmdSearch, show: cmdShow, get: cmdGet, sync: cmdSync, sources: cmdSources };

async function main() {
  const [name, ...rest] = process.argv.slice(2);
  if (!name || name === '--help' || name === '-h') return console.log(HELP);
  const command = COMMANDS[name];
  if (!command) throw new UsageError(`Unknown command: ${name}`);
  const parsed = parseArgs({
    args: rest,
    allowPositionals: true,
    options: {
      source: { type: 'string' }, limit: { type: 'string' }, details: { type: 'string' }, json: { type: 'boolean' }, project: { type: 'string' },
      code: { type: 'boolean' }, out: { type: 'string' }, force: { type: 'boolean' },
      global: { type: 'boolean' }, target: { type: 'string' }, 'no-sync': { type: 'boolean' }, 'available-only': { type: 'boolean' },
    },
  });
  await command(parsed);
}

main().catch((error) => {
  const usage = error instanceof UsageError || error.code?.startsWith?.('ERR_PARSE_ARGS');
  console.error(usage ? error.message.split('\n')[0] : `Error: ${error.message}`);
  process.exitCode = usage ? 2 : 1;
});
