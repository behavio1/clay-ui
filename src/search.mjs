// Development-only Node reference; the published launcher runs the Rust CLI.
// Query expansion and ranking over normalized candidates.

const ENGLISH_SYNONYMS = {
  carousel: ['slider'], toast: ['notification'], notification: ['toast'], drawer: ['sheet'], sheet: ['drawer'],
  modal: ['dialog'], dialog: ['modal'], palette: ['command'], counter: ['number'],
};

const STOPWORDS = new Set(['and', 'or', 'to', 'in', 'on', 'that', 'i', 'the', 'a', 'an', 'with', 'for', 'of', 'component', 'components']);

export function normalize(text) {
  return String(text ?? '').toLowerCase().normalize('NFD').replace(/\p{M}/gu, '');
}

const tokenize = (text) => normalize(text).split(/[^a-z0-9]+/).filter(Boolean);
const singular = (token) => (token.length > 3 && token.endsWith('s') ? token.slice(0, -1) : token);

// One group per query word; a candidate matches the word through any term in its group.
export function expandQuery(query) {
  return tokenize(query).filter((token) => !STOPWORDS.has(token)).map((token) => {
    return { token, synonyms: ENGLISH_SYNONYMS[singular(token)] ?? [] };
  });
}

function selectTop(ordered, { limit, perSource }) {
  const taken = new Map();
  const result = [];
  for (const { candidate, score } of ordered) {
    const count = taken.get(candidate.source) ?? 0;
    if (count >= perSource) continue;
    taken.set(candidate.source, count + 1);
    result.push({ ...candidate, score: Math.round(score * 1000) / 1000 });
    if (result.length >= limit) break;
  }
  return result;
}

const K1 = 1.2;
const B = 0.75;
const FIELD_WEIGHTS = [['name', 2], ['title', 1], ['keywords', 1], ['description', 1]];

function weightedTerms(candidate) {
  const counts = new Map();
  let length = 0;
  for (const [field, weight] of FIELD_WEIGHTS) {
    const value = Array.isArray(candidate[field]) ? candidate[field].join(' ') : candidate[field];
    for (const token of tokenize(value).map(singular)) {
      counts.set(token, (counts.get(token) ?? 0) + weight);
      length += weight;
    }
  }
  return { counts, length };
}

// Offline fallback: BM25 over name, title, keywords and description.
export function rank(candidates, query, { limit = 20, perSource = 5 } = {}) {
  const terms = [...new Set(expandQuery(query).flatMap(({ token, synonyms }) => [token, ...synonyms]).map(singular))];
  if (!terms.length || !candidates.length) return [];
  const documents = candidates.map((candidate) => ({ candidate, ...weightedTerms(candidate) }));
  const average = documents.reduce((sum, document) => sum + document.length, 0) / documents.length || 1;
  const idf = new Map(terms.map((term) => {
    const found = documents.filter(({ counts }) => counts.has(term)).length;
    return [term, Math.log(1 + (documents.length - found + 0.5) / (found + 0.5))];
  }));
  const ordered = documents
    .map(({ candidate, counts, length }) => ({
      candidate,
      score: terms.reduce((sum, term) => {
        const frequency = counts.get(term) ?? 0;
        return sum + idf.get(term) * (frequency * (K1 + 1)) / (frequency + K1 * (1 - B + B * (length / average)));
      }, 0),
    }))
    .filter(({ score }) => score > 0)
    .sort((a, b) => b.score - a.score || a.candidate.id.localeCompare(b.candidate.id));
  return selectTop(ordered, { limit, perSource });
}

// Weights chosen on the dev split of docs/research/search-eval.json (e2e/eval.mjs).
// minSimilarity sits between off-topic queries (max 0.59) and the weakest relevant answer (0.64).
export const RANKING = { gapPenalty: 0.08, curatedBonus: 0.04, minSimilarity: 0.6 };

// Gaps that stop the code from being downloaded. Cosmetic gaps such as a missing demo page never affect order.
const CODE_BLOCKING = new Set(['needs-key', 'pro-only', 'no-source-listing']);

// Primary ranking: cosine similarity.
export function rankSemantic(candidates, similarity, { limit = 20, perSource = 5, gapPenalty = RANKING.gapPenalty, curatedBonus = RANKING.curatedBonus, minSimilarity = RANKING.minSimilarity } = {}) {
  const ordered = candidates
    .map((candidate) => {
      const value = similarity.get(candidate.id) ?? 0;
      return { candidate, value, score: value - (candidate.gaps.some((gap) => CODE_BLOCKING.has(gap)) ? gapPenalty : 0) + (candidate.curated ? curatedBonus : 0) };
    })
    .filter(({ value }) => value >= minSimilarity)
    .sort((a, b) => b.score - a.score || a.candidate.id.localeCompare(b.candidate.id));
  return selectTop(ordered, { limit, perSource });
}
