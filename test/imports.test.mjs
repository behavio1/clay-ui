import assert from 'node:assert/strict';
import { test } from 'node:test';
import { missingLocalImports } from '../src/imports.mjs';

test('local import inventory resolves delivered files and flags lazy siblings without executing code', () => {
  const files = [
    { path: 'src/main.tsx', content: `import type { A } from './a.js'; export { B } from './folder'; import './style.css'; const lazy = import('../LiquidWave'); const c = require('./absent'); import './side'; // import './comment'
/* from './comment2' */ const str = "from './string'"; import('pkg'); import('@/alias');` },
    ...['src/a.ts', 'src/folder/index.tsx', 'src/style.css'].map(path => ({ path, content: ' ' })),
  ];
  assert.deepEqual(missingLocalImports(files), ['../LiquidWave', './absent', './side', '@/alias'].map(specifier => ({ file: 'src/main.tsx', import: specifier })));
});
