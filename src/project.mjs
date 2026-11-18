// Development-only Node reference; the published launcher runs the Rust CLI.
// Reads the target project and reports what an example would need there. Never writes to the project.
import { existsSync, readFileSync } from 'node:fs';
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

export function missingDependencies(detail, project) {
  const packages = importSpecifiers(detail.files ?? []).map(m => m.import).filter(s => !/^[./#]|^@\/|:/.test(s)).map(s => s.split('/').slice(0, s.startsWith('@') ? 2 : 1).join('/'));
  const list = [...(detail.dependencies ?? [])];
  for (const pkg of packages) if (!list.some(d => d.replace(/(?!^)@.*$/, '') === pkg)) list.push(pkg);
  return list.filter(d => d !== 'cn' && !(d.replace(/(?!^)@.*$/, '') in project.dependencies));
}

// Relative output path for a downloaded file, or null when the registry path is unsafe.
export function safeRelative(value) {
  if (!value || path.isAbsolute(value) || /^[A-Za-z]:/.test(value) || value.startsWith('~')) return null;
  const parts = value.split(/[\\/]/).filter(Boolean);
  return parts.length && !parts.some((part) => part === '..' || part === '.' || part.includes(':')) ? parts.join('/') : null;
}
