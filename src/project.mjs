// Development-only Node reference; the published launcher runs the Rust CLI.
// Reads the target project and reports what an example would need there. Never writes to the project.
import { existsSync, lstatSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { importSpecifiers } from './imports.mjs';

export class UsageError extends Error {}

export function inspectProject(directory) {
  const root = path.resolve(directory);
  const file = path.join(root, 'package.json');
  if (!existsSync(file)) throw new UsageError(`No package.json in ${root}`);
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(file, 'utf8'));
  } catch {
    throw new UsageError(`Niepoprawny package.json w ${root}`);
  }
  const dependencies = { ...manifest.dependencies, ...manifest.devDependencies };
  return { root, react: dependencies.react ?? null, tailwind: dependencies.tailwindcss ?? null, dependencies };
}

const bareName = (spec) => spec.replace(/(?!^)@.*$/, '');
const npmNamePart = (part) => part.length >= 1 && part.length <= 214 && !/^[._-]/.test(part) && /^[a-z0-9\-._~]+$/.test(part);
function semverPartial(text) {
  const match = /^[vV]?([^+-]*)(?:-([^+]*))?(?:\+(.*))?$/.exec(text);
  if (!match) return false;
  const tag = (value) => value === undefined || /^[0-9A-Za-z.-]+$/.test(value);
  const parts = match[1].split('.');
  return parts.length >= 1 && parts.length <= 3 && parts.every((part) => /^(x|X|\*|\d+)$/.test(part)) && tag(match[2]) && tag(match[3]);
}
const semverComparator = (text) => semverPartial(text.replace(/^(>=|<=|>|<|=|\^|~)/, ''));
function npmVersion(version) {
  if (/^[A-Za-z][A-Za-z0-9._-]*$/.test(version) && !/^[vV]\d/.test(version)) return true;
  return version.split('||').every((part) => {
    const set = part.trim();
    const hyphen = set.indexOf(' - ');
    if (hyphen >= 0) return semverPartial(set.slice(0, hyphen).trim()) && semverPartial(set.slice(hyphen + 3).trim());
    return set.length > 0 && set.split(/\s+/).every(semverComparator);
  });
}

// A registry dependency npm resolves by name; aliases, URLs, git, file paths and archives are not plain.
export function plainDependency(spec) {
  if (typeof spec !== 'string') return false;
  const name = bareName(spec);
  if (spec.length > name.length && !npmVersion(spec.slice(name.length + 1))) return false;
  const [scope, part] = name.startsWith('@') ? [name.slice(1).split('/')[0], name.slice(1).split('/').slice(1).join('/')] : [null, name];
  const valid = scope === null ? npmNamePart(name) : npmNamePart(scope) && npmNamePart(part);
  return valid && !/\.(tgz|tar|tar\.gz)$/.test(name);
}

// Files get actually writes: safe paths that are not agent instruction files.
export const deliverableFiles = (detail) => (detail.files ?? []).filter((file) => {
  const relative = safeRelative(file.target || file.path);
  return !relative || !agentInstructionPath(`${detail.name ?? ''}/${relative}`);
});

function dependencyLists(detail) {
  const packages = importSpecifiers(deliverableFiles(detail)).map(m => m.import).filter(s => !/^[./#]|^@\/|:/.test(s)).map(s => s.split('/').slice(0, s.startsWith('@') ? 2 : 1).join('/')).filter(s => s !== 'cn');
  const plain = [], unverified = [], listed = new Set(), seen = new Set();
  const upstream = detail.dependencies ?? [];
  for (const [index, value] of [...upstream, ...packages].entries()) {
    if (plainDependency(value)) {
      // Upstream entries stay as listed; an import is added only for a name not yet listed.
      const fresh = !listed.has(bareName(value));
      listed.add(bareName(value));
      if (fresh || index < upstream.length) plain.push(value);
    } else if (!seen.has(JSON.stringify(value))) { seen.add(JSON.stringify(value)); unverified.push(value); }
  }
  return { plain, unverified };
}

// Plain npm dependencies the project does not list, and the ones that are not plain npm names.
export function dependenciesFor(detail, project) {
  const { plain, unverified } = dependencyLists(detail);
  return { missing: plain.filter(d => bareName(d) !== 'cn' && !(bareName(d) in project.dependencies)), unverified };
}
export const missingDependencies = (detail, project) => dependenciesFor(detail, project).missing;

const INSTRUCTION_FOLDERS = ['.claude', '.agents', '.cursor', '.codex', '.windsurf', '.gemini', '.clinerules', '.roo', '.kiro'];
const INSTRUCTION_FILES = ['claude.md', 'claude.local.md', 'agents.md', 'gemini.md', '.cursorrules', '.windsurfrules', '.clinerules', 'copilot-instructions.md'];
// True for a path coding agents load as instructions, rules or skills.
export function agentInstructionPath(value) {
  const parts = value.split(/[\\/]/).filter(Boolean).map((part) => part.toLowerCase());
  const file = parts.pop();
  if (file === undefined) return false;
  return parts.some((part) => INSTRUCTION_FOLDERS.includes(part)) || INSTRUCTION_FILES.includes(file) || INSTRUCTION_FOLDERS.includes(file) || /\.(mdc|instructions\.md|prompt\.md|chatmode\.md)$/.test(file);
}

// Relative output path for a downloaded file, or null when the registry path is unsafe.
export function safeRelative(value) {
  if (!value || path.isAbsolute(value) || /^[A-Za-z]:/.test(value) || value.startsWith('~')) return null;
  const parts = value.split(/[\\/]/).filter(Boolean);
  return parts.length && !parts.some((part) => part === '..' || part === '.' || part.includes(':')) ? parts.join('/') : null;
}

// First agent instruction file or folder inside folder, without following links.
// A folder that does not exist has none; a folder that cannot be fully read throws.
export function findAgentInstruction(folder) {
  let stat;
  try { stat = lstatSync(folder); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
  if (stat.isSymbolicLink()) return null; // never scanned; the writer refuses a linked destination

  const stack = [folder];
  while (stack.length) {
    const dir = stack.pop();
    for (const name of readdirSync(dir)) {
      const full = path.join(dir, name);
      if (agentInstructionPath(path.relative(folder, full))) return full;
      if (lstatSync(full).isDirectory()) stack.push(full);
    }
  }
  return null;
}
