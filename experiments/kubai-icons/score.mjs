// Scores result files with the frozen dev scorer (read-only) and compares each to the first.
// usage: node score.mjs <base.results.json> <other.results.json>...
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const EVAL = '/Users/Marcin_1/Documents/Behavio.one/Projects/ui-component-finder/eval';
const { evaluate } = await import(pathToFileURL(`${EVAL}/score.mjs`));
const data = JSON.parse(readFileSync(`${EVAL}/dev.json`));
data.queries = data.queries.filter((q) => q.lang === 'en');
const read = (file) => JSON.parse(readFileSync(file));
const files = process.argv.slice(2);
const base = read(files[0]);
const baseById = new Map(base.results.map((r) => [r.id, r.components]));
for (const file of files) {
  const input = read(file);
  const report = evaluate(data, input);
  const c = report.overall.conservative, o = report.overall.optimistic;
  let changed = 0, kubai = 0, queriesWithKubai = 0;
  for (const r of input.results) {
    if (JSON.stringify(r.components) !== JSON.stringify(baseById.get(r.id))) changed += 1;
    const k = r.components.filter((id) => id.startsWith('kubai-icons/')).length;
    kubai += k;
    if (k) queriesWithKubai += 1;
  }
  console.log(JSON.stringify({
    file,
    success3at5: { conservative: c.success3at5, optimistic: o.success3at5 },
    contextSuccess3at5: { conservative: c.contextSuccess3at5, optimistic: o.contextSuccess3at5 },
    correctRejection: c.correctRejection,
    unjudgedTop5: c.unjudgedTop5Count,
    queriesChangedVsFirst: changed,
    kubaiSlots: kubai,
    queriesWithKubai,
  }));
}
