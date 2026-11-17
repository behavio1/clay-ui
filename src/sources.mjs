// Development-only Node reference; the published launcher runs the Rust CLI.
// Component libraries as data, plus reading each one into the shared candidate shape.

export const TIMEOUT_MS = 15_000;
const KOKONUT_RAW = 'https://raw.githubusercontent.com/kokonut-labs/kokonutui/main/public/r';
const SAFE_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const COMPONENT_TYPES = new Set(['registry:ui', 'registry:component', 'registry:block']);

export class SourceError extends Error {}

async function request(url, init = {}) {
  try {
    return await fetch(url, { redirect: 'follow', signal: AbortSignal.timeout(TIMEOUT_MS), ...init });
  } catch (error) {
    throw new SourceError(`${new URL(url).host}: ${error.cause?.code ?? error.name}`);
  }
}

async function fetchOk(url, init) {
  const response = await request(url, init);
  if (!response.ok) throw Object.assign(new SourceError(`${new URL(url).host}: HTTP ${response.status}`), { status: response.status });
  return response;
}

const fetchText = async (url, init) => (await fetchOk(url, init)).text();

async function fetchJson(url, init) {
  const text = await fetchText(url, init);
  try {
    return JSON.parse(text);
  } catch {
    throw new SourceError(`${new URL(url).host}: invalid JSON`);
  }
}

async function mapLimit(items, limit, worker) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const index = next;
      next += 1;
      results[index] = await worker(items[index]);
    }
  }));
  return results;
}

const frontmatter = (text, key) => text.match(new RegExp(`^${key}:\\s*(.+)$`, 'm'))?.[1]?.trim().replace(/^["']|["']$/g, '') ?? '';
const githubHeaders = () => (process.env.GITHUB_TOKEN ? { headers: { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } } : undefined);
const repoTree = async (repo, ref = 'HEAD') => (await fetchJson(`https://api.github.com/repos/${repo}/git/trees/${ref}?recursive=1`, githubHeaders())).tree ?? [];

const titleFromSlug = (slug) => slug.replace(/-/g, ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());

function candidate(source, fields) {
  return {
    id: `${source.id}/${fields.name}`,
    source: source.id,
    title: titleFromSlug(fields.name),
    description: '',
    previewUrl: null,
    license: source.license ?? null,
    dependencies: [],
    registryDependencies: [],
    gaps: [],
    ...fields,
  };
}

function withGaps(item, extra = []) {
  const gaps = new Set([...item.gaps, ...extra]);
  if (!item.license) gaps.add('no-license');
  if (!item.previewUrl) gaps.add('no-preview');
  if (item.previewUrl?.includes('#')) gaps.add('preview-unverified');
  return { ...item, gaps: [...gaps] };
}

// --- shadcn-compatible registries -----------------------------------------

const gapForStatus = (status) => [status === 404 ? 'no-source-listing' : 'pro-only'];

// Some index entries have no public code: paid items answer 401/403, retired ones 404.
async function codeGap(url) {
  try {
    const response = await request(url);
    return [401, 403, 404].includes(response.status) ? gapForStatus(response.status) : [];
  } catch {
    return [];
  }
}

function registrySource({ id, title, access = 'registry', license, indexUrl, itemUrl, previewUrl, previewMap, enrich, probeItems }) {
  const source = { id, title, access, license };
  source.previewFor = (item, map) => (previewMap ? map.get(item.name) ?? null : previewUrl(item));
  source.load = async () => {
    const [body, map] = await Promise.all([fetchJson(indexUrl), previewMap?.()]);
    const items = Array.isArray(body) ? body : body.items;
    if (!Array.isArray(items)) throw new SourceError(`${id}: unexpected index shape`);
    const components = items.filter((item) => (COMPONENT_TYPES.has(item.type) || (id === 'beautifului' && item.type === 'registry:style' && item.name === 'foundation')) && SAFE_NAME.test(item.name ?? ''));
    return mapLimit(components, 8, async (item) => {
      const preview = source.previewFor(item, map);
      const extra = enrich ? await enrich(item, preview).catch(() => ({})) : {};
      const gaps = probeItems ? await codeGap(itemUrl(item.name)) : [];
      return withGaps(candidate(source, {
        name: item.name,
        title: extra.title ?? item.title ?? titleFromSlug(item.name),
        description: extra.description ?? item.description ?? '',
        keywords: extra.keywords ?? [],
        previewUrl: preview,
        dependencies: item.dependencies ?? [],
        registryDependencies: item.registryDependencies ?? [],
      }), gaps);
    });
  };
  source.detail = async (item) => {
    let entry;
    try {
      entry = await fetchJson(itemUrl(item.name));
    } catch (error) {
      if (![401, 403, 404].includes(error.status)) throw error;
      return withGaps({ ...item, files: [] }, gapForStatus(error.status));
    }
    return {
      ...item,
      sourceUrl: itemUrl(item.name),
      ...(entry.version ? { version: entry.version } : {}),
      ...(entry.ref ? { sourceRef: entry.ref } : {}),
      ...(id === 'kokonut' ? { sourceRef: 'main' } : {}),
      description: entry.description ?? item.description,
      dependencies: entry.dependencies ?? [],
      registryDependencies: entry.registryDependencies ?? [],
      envVars: Object.keys(entry.envVars ?? {}),
      files: (entry.files ?? []).map(({ path, target, content }) => ({ path, target, content })),
    };
  };
  return source;
}

const kokonutPreviews = async () => {
  const tree = await fetchJson('https://api.github.com/repos/kokonut-labs/kokonutui/git/trees/main?recursive=1', githubHeaders());
  const map = new Map();
  for (const { path } of tree.tree ?? []) {
    const match = path.match(/^content\/docs\/([^/]+)\/([^/]+)\.mdx$/);
    if (match) map.set(match[2], `https://kokonutui.com/docs/${match[1]}/${match[2]}`);
  }
  return map;
};

// --- npm packages ---------------------------------------------------------

async function npmLicense(pkg) {
  const meta = await fetchJson(`https://registry.npmjs.org/${pkg.replace('/', '%2F')}/latest`);
  return typeof meta.license === 'string' ? meta.license : null;
}

const SOURCE_FILE = /\.(tsx?|css)$/;
const NOT_SOURCE = /(\.(test|spec|stories)\.)|(__tests__|\/tests?\/|\/stories\/)/;
const MAX_SOURCE_FILES = 120;

async function repoSourceFiles(sourceCode) {
  if (!sourceCode?.repo || !sourceCode?.dir) throw new SourceError('source location missing');
  const { repo, dir, include = [] } = sourceCode;
  const commit = await fetchJson(`https://api.github.com/repos/${repo}/commits/HEAD`, githubHeaders());
  const sourceRef = commit.sha ?? 'HEAD';
  const tree = await fetchJson(`https://api.github.com/repos/${repo}/git/trees/${sourceRef}?recursive=1`, githubHeaders());
  if (tree.truncated) throw new SourceError('GitHub source tree is truncated');
  const prefix = `${dir}/`;
  const paths = [], skipped = [];
  for (const node of tree.tree ?? []) {
    const file = node.path;
    if (!file?.startsWith(prefix) || node.type === 'tree') continue;
    // Optional sub-paths of `dir`; files outside them are neither fetched nor listed as skipped.
    if (include.length && !include.some((part) => file.slice(prefix.length).startsWith(part))) continue;
    if (SOURCE_FILE.test(file) && !NOT_SOURCE.test(file) && paths.length < MAX_SOURCE_FILES) paths.push(file);
    else skipped.push(`source/${file.slice(prefix.length)}`);
  }
  const files = await mapLimit(paths, 8, async (file) => ({
    path: `source/${file.slice(prefix.length)}`,
    content: await fetchText(`https://raw.githubusercontent.com/${repo}/${sourceRef}/${file.split('/').map(encodeURIComponent).join('/')}`),
  }));
  return { files, skipped, sourceUrl: `https://github.com/${repo}/tree/${sourceRef}/${dir}`, sourceRef };
}

// Libraries distributed through npm: original source from their repository plus the documented usage.
async function packageDetail(item) {
  const [usage, files] = await Promise.allSettled([fetchText(item.usageUrl), repoSourceFiles(item.sourceCode)]);
  const sourceWarnings = [usage, files].filter((result) => result.status === 'rejected').map((result) => result.reason.message);
  const detail = { ...item, usage: usage.status === 'fulfilled' ? usage.value : undefined, ...(files.status === 'fulfilled' ? { sourceUrl: files.value.sourceUrl, sourceRef: files.value.sourceRef } : {}), skipped: files.status === 'fulfilled' ? files.value.skipped : [], files: files.status === 'fulfilled' ? files.value.files : [], sourceWarnings };
  if (detail.files.length) return detail;
  return withGaps(detail, [detail.usage ? 'usage-only' : 'no-source-listing']);
}

function singlePackage({ id, title, pkg, docs, description, keywords, curated, unmaintained, sourceCode }) {
  const source = { id, title, access: 'npm package', detail: packageDetail };
  source.load = async () => {
    const license = await npmLicense(pkg);
    return [withGaps(candidate(source, {
      name: id, title, description, keywords, license, curated, sourceCode, previewUrl: docs, dependencies: [pkg],
      usageUrl: `https://unpkg.com/${pkg}/README.md`,
    }), unmaintained ? ['unmaintained'] : [])];
  };
  return source;
}

function radixSource() {
  const source = { id: 'radix', title: 'Radix Primitives', access: 'repo docs + npm package', detail: packageDetail };
  const raw = 'https://raw.githubusercontent.com/radix-ui/website/main';
  source.load = async () => {
    const [tree, license] = await Promise.all([repoTree('radix-ui/website'), npmLicense('radix-ui')]);
    const files = tree.map((node) => node.path).filter((file) => /^data\/primitives\/docs\/components\/[a-z0-9-]+\.mdx$/.test(file));
    if (!files.length) throw new SourceError('radix: no component docs in repo tree');
    return mapLimit(files, 8, async (file) => {
      const name = file.split('/').pop().replace('.mdx', '');
      const usageUrl = `${raw}/${file}`;
      return withGaps(candidate(source, {
        name, license, usageUrl, sourceCode: { repo: 'radix-ui/primitives', dir: `packages/react/${name}/src` }, description: frontmatter(await fetchText(usageUrl), 'metaDescription'), keywords: ['primitive', 'unstyled'],
        previewUrl: `https://www.radix-ui.com/primitives/docs/components/${name}`, dependencies: ['radix-ui'],
      }));
    });
  };
  return source;
}

// --- UI SDK (Vercel Labs) -------------------------------------------------

function uiSdkSource() {
  const source = { id: 'ui-sdk', title: 'UI SDK (Vercel Labs)', access: 'repo docs + examples', detail: packageDetail };
  const repo = 'vercel-labs/ui';
  source.load = async () => {
    const [tree, license] = await Promise.all([repoTree(repo), npmLicense('@ui-sdk/react')]);
    const paths = tree.map((node) => node.path);
    const files = paths.filter((file) => /^apps\/docs\/content\/docs\/react\/components\/[a-z0-9-]+\.mdx$/.test(file) && !file.endsWith('/index.mdx'));
    if (!files.length) throw new SourceError('ui-sdk: no component docs in repo tree');
    return mapLimit(files, 8, async (file) => {
      const name = file.split('/').pop().replace('.mdx', '');
      const text = await fetchText(`https://raw.githubusercontent.com/${repo}/main/${file}`);
      const page = `https://ui-sdk.dev/docs/react/components/${name}`;
      // Without React examples there is nothing to copy beyond the docs page.
      const hasExamples = paths.some((path) => path?.startsWith(`examples/components/${name}/react/`));
      return withGaps(candidate(source, {
        name, title: frontmatter(text, 'title') || titleFromSlug(name), description: frontmatter(text, 'description'), license, usageUrl: `${page}.md`,
        ...(hasExamples ? { sourceCode: { repo, dir: 'examples', include: [`components/${name}/react/`, `components/${name}/${name}.css`, 'shared/react/', 'styles/'] } } : {}),
        keywords: ['primitive', 'unstyled', 'headless'], previewUrl: page, dependencies: ['@ui-sdk/react', '@ui-sdk/core'],
      }), ['beta']);
    });
  };
  return source;
}

const BASE_UI_CURATED = new Set(['dialog', 'menu', 'popover', 'select']);

function baseUiSource() {
  const source = { id: 'base-ui', title: 'Base UI', access: 'llms.txt + npm package', detail: packageDetail };
  source.load = async () => {
    const [text, license] = await Promise.all([fetchText('https://base-ui.com/llms.txt'), npmLicense('@base-ui/react')]);
    const section = text.split(/^## Components$/m)[1]?.split(/^## /m)[0];
    if (!section) throw new SourceError('base-ui: components section missing');
    return [...section.matchAll(/^- \[([^\]]+)\]\((https:\/\/base-ui\.com\/react\/components\/([a-z0-9-]+))\.md\):\s*(.*)$/gm)]
      .map(([, title, url, name, description]) => withGaps(candidate(source, {
        name, title, description, license, previewUrl: url, usageUrl: `${url}.md`, sourceCode: { repo: 'mui/base-ui', dir: `packages/react/src/${name}` }, keywords: ['primitive', 'unstyled'],
        curated: BASE_UI_CURATED.has(name) ? 'pick-ui-library' : undefined,
        dependencies: ['@base-ui/react'],
      })));
  };
  return source;
}

// --- RareUI ---------------------------------------------------------------

function rareUiSource() {
  const source = { id: 'rareui', title: 'ShitUI (formerly RareUI)', access: 'registry + repo docs', license: 'MIT' };
  const squash = (text) => text.toLowerCase().replace(/[^a-z0-9]/g, '');
  const registryItem = (registryName) => `https://shitui.in/r/${encodeURIComponent(registryName)}.json`;
  source.load = async () => {
    const [index, tree] = await Promise.all([fetchJson('https://shitui.in/r/index.json'), repoTree('Codewithswappy/ShitUI')]);
    if (!Array.isArray(index.components)) throw new SourceError('rareui: unexpected index shape');
    const docs = new Map();
    for (const { path: file } of tree) {
      const match = file.match(/^content\/docs\/([^/]+)\/([^/]+)\.mdx$/);
      if (match) docs.set(squash(match[2]), { category: match[1], slug: match[2], file });
    }
    return mapLimit(index.components.filter((name) => SAFE_NAME.test(name)), 8, async (registryName) => {
      const doc = docs.get(squash(registryName));
      const text = doc ? await fetchText(`https://raw.githubusercontent.com/Codewithswappy/ShitUI/main/${doc.file.split('/').map(encodeURIComponent).join('/')}`).catch(() => '') : '';
      return withGaps(candidate(source, {
        name: registryName,
        title: frontmatter(text, 'title') || titleFromSlug(registryName.replace(/([a-z])([A-Z])/g, '$1-$2')),
        description: frontmatter(text, 'description'),
        keywords: doc ? [doc.category] : [],
        previewUrl: doc ? `https://shitui.in/docs/${doc.category}/${doc.slug}` : null,
      }));
    });
  };
  source.detail = async (item) => {
    const entry = await fetchJson(registryItem(item.name));
    return { ...item, sourceUrl: registryItem(item.name), ...(entry.version ? { version: entry.version } : {}), ...(entry.ref ? { sourceRef: entry.ref } : {}), dependencies: entry.dependencies ?? [], registryDependencies: entry.registryDependencies ?? [], files: (entry.files ?? []).map(({ path, target, content }) => ({ path, target, content })) };
  };
  return source;
}

// --- 21st.dev (live; the catalog is too large to index) -------------------

// Component code on 21st needs a key; without one only metadata is public.
async function files21st(user, slug, key = process.env.API_KEY_21ST) {
  if (!key) return [];
  const body = await fetchJson(`https://21st.dev/api/v1/components/install/${encodeURIComponent(user)}/${encodeURIComponent(slug)}`, { headers: { Authorization: `Bearer ${key}` } });
  return (body.files ?? []).map(({ path, target, content }) => ({ path, target, content }));
}

// Owner's rule (2026-11-08): a 21st licence of "unknown" is treated as MIT, flagged as assumed.
const ASSUMED_MIT = { license: 'MIT', licenseAssumed: true };

function license21st(text) {
  const stated = text.match(/^- License: (.*)$/m)?.[1]?.trim();
  const known = stated && stated.toLowerCase() !== 'unknown';
  return { license: known ? (stated.toLowerCase() === 'mit' ? 'MIT' : stated) : 'MIT', licenseAssumed: !known };
}
export async function searchLicense(item) {
  if ((item.source === '21st' || item.mirror) && item.previewUrl) {
    try { return { ...item, ...license21st(await fetchText(item.previewUrl.replace(/\/$/, '') + '.md')) }; } catch {}
  }
  return item;
}

async function detail21st(item, user, slug) {
  const sourceUrl = `https://21st.dev/@${encodeURIComponent(user)}/components/${encodeURIComponent(slug)}.md`;
  const unavailable = () => withGaps({ ...item, sourceUrl, files: [], sourceWarnings: ['Upstream component is unavailable (404)'], gaps: item.gaps.filter(g => g !== 'needs-key') }, ['no-source-listing']);
  let text, files;
  try { text = await fetchText(sourceUrl); files = await files21st(user, slug); }
  catch (error) { if (error.status === 404) return unavailable(); throw error; }
  const field = (label) => text.match(new RegExp(`^- ${label}: (.+)$`, 'm'))?.[1]?.trim();
  return withGaps({
    ...item,
    title: text.match(/^# (.+)$/m)?.[1] ?? item.title,
    description: text.match(/^> (.+)$/m)?.[1] ?? item.description,
    ...license21st(text),
    dependencies: field('npm dependencies')?.split(/,\s*/) ?? [],
    sourceUrl, files,
    gaps: item.gaps.filter((gap) => gap !== 'no-license'),
  });
}

function source21st() {
  const source = { id: '21st', title: '21st.dev', access: 'live API / Markdown', live: true };

  const toCandidate = ({ user, slug, title, description = '', keywords = [] }, key) => withGaps(candidate(source, {
    ...ASSUMED_MIT,
    name: `${user}/${slug}`, title, description, keywords,
    previewUrl: `https://21st.dev/@${user}/components/${slug}`,
  }), key ? [] : ['needs-key']);

  source.searchLive = async (groups, { key = process.env.API_KEY_21ST, limit = 25 } = {}) => {
    const warnings = [];
    const results = new Map();
    if (key) {
      const query = groups.map((group) => group.synonyms[0] ?? group.token).join(' ');
      const body = await fetchJson(`https://21st.dev/api/v1/components/search?q=${encodeURIComponent(query)}&scope=public&limit=${Math.min(limit, 50)}`, { headers: { Authorization: `Bearer ${key}` } });
      for (const row of body.results ?? []) {
        const match = String(row.url ?? '').match(/@([\w.-]+)\/components\/([\w.-]+)/);
        if (match) results.set(`${match[1]}/${match[2]}`, toCandidate({ user: match[1], slug: match[2], title: row.name ?? match[2], description: row.description ?? '', keywords: row.tags ?? [] }, key));
      }
      return { items: [...results.values()], warnings };
    }
    const tags = [...new Set(groups.flatMap((group) => [group.token, ...group.synonyms]))].slice(0, 5);
    let found = 0;
    await Promise.all(tags.map(async (tag) => {
      const response = await request(`https://21st.dev/community/components/s/${encodeURIComponent(tag)}.md`);
      if (!response.ok) return;
      found += 1;
      const text = await response.text();
      for (const [, title, user, slug] of text.matchAll(/^- \[([^\]]+)\]\(https:\/\/21st\.dev\/@([\w.-]+)\/components\/([\w.-]+)(?:\/[\w.-]+)?\.md\)/gm)) {
        const id = `${user}/${slug}`;
        const existing = results.get(id);
        if (existing) existing.keywords.push(tag);
        else results.set(id, toCandidate({ user, slug, title, keywords: [tag] }, key));
      }
    }));
    if (found === 0) throw new SourceError(`21st: no tag page for ${tags.join(', ')}`);
    warnings.push('21st.dev without a key: tag search only, up to 60 results per tag');
    return { items: [...results.values()], warnings };
  };

  source.fromId = (name, key = process.env.API_KEY_21ST) => {
    const [user, slug] = name.split('/');
    if (!user || !slug || name.split('/').length !== 2) return null;
    return toCandidate({ user, slug, title: titleFromSlug(slug) }, key);
  };

  source.detail = (item) => detail21st(item, ...item.name.split('/'));
  return source;
}

async function loadAceternityCatalog() {
  const body = await fetchJson('https://ui.aceternity.com/api/components');
  const categories = new Map();
  for (const [category, names] of Object.entries(body.categories ?? {})) for (const name of names) categories.set(name, [...(categories.get(name) ?? []), category]);
  return new Map([...(body.components ?? []), ...(body.blocks ?? [])].map((entry) => [entry.name, { title: entry.title, description: entry.description ?? '', keywords: categories.get(entry.name) ?? [], documentationUrl: entry.documentationUrl }]));
}

// Official registry when reachable; otherwise the author's 21st library, marked as a partial mirror.
function aceternitySource() {
  const owner = 'manuarora700';
  let catalog = new Map();
  const source = registrySource({
    id: 'aceternity', title: 'Aceternity UI', license: 'Aceternity License', probeItems: true, indexUrl: 'https://ui.aceternity.com/registry/index.json',
    itemUrl: (name) => `https://ui.aceternity.com/registry/${encodeURIComponent(name)}.json`,
    previewMap: async () => {
      catalog = await loadAceternityCatalog().catch(() => new Map());
      return new Map([...catalog].filter(([, entry]) => entry.documentationUrl).map(([name, entry]) => [name, entry.documentationUrl]));
    },
    enrich: async (item) => catalog.get(item.name) ?? {},
  });
  const official = { load: source.load, detail: source.detail };
  source.load = async () => {
    try {
      return await official.load();
    } catch (error) {
      if (!(error instanceof SourceError)) throw error;
      const text = await fetchText(`https://21st.dev/@${owner}/library/aceternity-ui.md`);
      return [...text.matchAll(/^- \[([^\]]+)\]\(https:\/\/21st\.dev\/@manuarora700\/components\/([\w.-]+)\.md\)/gm)].map(([, title, slug]) => withGaps(candidate(source, {
        ...ASSUMED_MIT, name: slug, title, mirror: true, previewUrl: `https://21st.dev/@${owner}/components/${slug}`,
      }), ['partial-mirror', 'needs-key']));
    }
  };
  source.detail = (item) => (item.mirror ? detail21st(item, owner, item.name) : official.detail(item));
  return source;
}

// --- The catalog ----------------------------------------------------------

export const SOURCES = [
  registrySource({
    id: 'shadcn', title: 'shadcn/ui', license: 'MIT', probeItems: true, indexUrl: 'https://ui.shadcn.com/r/index.json',
    itemUrl: (name) => `https://ui.shadcn.com/r/styles/new-york-v4/${encodeURIComponent(name)}.json`,
    previewUrl: (item) => item.meta?.links?.radix?.docs ?? item.meta?.links?.base?.docs ?? null,
    enrich: async (item, preview) => (preview ? { description: frontmatter(await fetchText(`${preview}.md`), 'description') } : {}),
  }),
  registrySource({
    id: 'magicui', title: 'Magic UI', license: 'MIT', indexUrl: 'https://magicui.design/r/registry.json',
    itemUrl: (name) => `https://magicui.design/r/${encodeURIComponent(name)}.json`,
    previewUrl: (item) => `https://magicui.design/docs/components/${item.name}`,
  }),
  aceternitySource(),
  registrySource({
    id: 'kokonut', title: 'Kokonut UI', access: 'registry via GitHub', license: 'MIT', indexUrl: `${KOKONUT_RAW}/registry.json`,
    itemUrl: (name) => `${KOKONUT_RAW}/${encodeURIComponent(name)}.json`,
    previewMap: kokonutPreviews,
  }),
  registrySource({
    id: 'beautifului', title: 'Beautiful UI', license: 'MIT', indexUrl: 'https://www.beautifului.dev/r/registry.json',
    itemUrl: (name) => `https://www.beautifului.dev/r/${encodeURIComponent(name)}.json`,
    previewUrl: (item) => (item.type === 'registry:component' ? `https://www.beautifului.dev/#${item.name}` : null),
    enrich: async (item) => ({ description: (await fetchJson(`https://www.beautifului.dev/r/${encodeURIComponent(item.name)}.json`)).description }),
  }),
  source21st(),
  rareUiSource(),
  baseUiSource(),
  radixSource(),
  uiSdkSource(),
  singlePackage({ id: 'sonner', sourceCode: { repo: 'emilkowalski/sonner', dir: 'src' }, title: 'Sonner', pkg: 'sonner', docs: 'https://sonner.emilkowal.ski', description: 'An opinionated toast notification component for React.', keywords: ['toast', 'notification', 'alert', 'snackbar'], curated: 'pick-ui-library' }),
  singlePackage({ id: 'vaul', sourceCode: { repo: 'emilkowalski/vaul', dir: 'src' }, title: 'Vaul', pkg: 'vaul', docs: 'https://vaul.emilkowal.ski', description: 'Unstyled drawer and bottom sheet component for React.', keywords: ['drawer', 'bottom', 'sheet', 'mobile'], unmaintained: true }),
  singlePackage({ id: 'cmdk', sourceCode: { repo: 'dip/cmdk', dir: 'cmdk/src' }, title: 'cmdk', pkg: 'cmdk', docs: null, description: 'Fast, composable command menu for React (Cmd+K palette). No live demo page: the old site redirects to the GitHub repository.', keywords: ['command', 'menu', 'palette', 'search', 'combobox', 'cmdk'], curated: 'pick-ui-library' }),
  singlePackage({ id: 'number-flow', sourceCode: { repo: 'barvian/number-flow', dir: 'packages/react/src' }, title: 'NumberFlow', pkg: '@number-flow/react', docs: 'https://number-flow.barvian.me', description: 'Animated number component with digit transitions.', keywords: ['number', 'counter', 'animated', 'price', 'stat'], curated: 'pick-ui-library' }),
];

export const sourceById = (id) => SOURCES.find((source) => source.id === id);

// ok = page exists, broken = page is gone, unverified = could not tell, none = no page known.
export async function previewStatus(url) {
  if (!url) return 'none';
  if (url.includes('#')) return 'unverified';
  try {
    const response = await request(url);
    if (new URL(response.url).host !== new URL(url).host) return 'unverified';
    if (response.ok) return 'ok';
    return response.status === 404 || response.status === 410 ? 'broken' : 'unverified';
  } catch {
    return 'unverified';
  }
}
