// Development-only counterpart of imports.rs; inspects delivered data only.
import path from 'node:path';
import { safeRelative } from './project.mjs';

export function importSpecifiers(entries) {
  const files = entries.filter(f => typeof f.content === 'string' && f.content.length)
    .map(f => ({ file: safeRelative(f.target || f.path), content: f.content })).filter(f => f.file);
  const missing = [];
  for (const { file, content } of files) {
    if (!/\.(tsx?|jsx?|[mc][tj]s)$/.test(file)) continue;
    const tokens = [...content.matchAll(/\/\/[^\n]*|\/\*[\s\S]*?\*\/|'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`|[\p{L}_$][\p{L}\p{N}_$]*|[^\s]/gu)]
      .map(m => m[0]).filter(t => !t.startsWith('//') && !t.startsWith('/*'));
    for (let i = 0; i < tokens.length; i++) {
      const word = tokens[i];
      const literal = ['import', 'require'].includes(word) && tokens[i + 1] === '(' ? tokens[i + 2]
        : ['import', 'from'].includes(word) ? tokens[i + 1] : null;
      if (!literal || !/^['"`]/.test(literal) || literal.includes('${')) continue;
      const specifier = literal.slice(1, -1);
      if (!missing.some(m => m.file === file && m.import === specifier)) missing.push({ file, import: specifier });
    }
  }
  return missing;
}

export function missingLocalImports(entries) {
  const delivered = new Set(entries.filter(f => f.content).map(f => safeRelative(f.target || f.path)).filter(Boolean));
  return importSpecifiers(entries).filter(({file, import: specifier}) => {
      if (!specifier.startsWith('./') && !specifier.startsWith('../') && !specifier.startsWith('@/')) return false;
      const bases = specifier.startsWith('@/') ? [specifier.slice(2), 'src/' + specifier.slice(2)] : [path.posix.normalize(path.posix.join(path.posix.dirname(file), specifier))];
      return !bases.some(resolved => {
      const stem = resolved.replace(/\.(jsx?|[mc]js)$/, '');
      const candidates = [resolved, ...['.ts', '.tsx', '.mts', '.cts'].map(ext => stem + ext),
        ...['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.mts', '.cts', '.json', '.css', '/index.ts', '/index.tsx', '/index.js', '/index.jsx', '/index.mjs', '/index.cjs'].map(ext => resolved + ext)];
      return candidates.some(c => delivered.has(c));
      });
  });
}
