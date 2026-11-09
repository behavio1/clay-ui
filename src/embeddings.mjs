// Development-only Node reference; the published launcher runs the Rust CLI.
// Gemini embeddings with a local vector cache. The key is read at runtime and only ever sent as a header.
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export const MODEL = process.env.CLAYUI_EMBED_MODEL ?? 'gemini-embedding-001';
const DIMENSIONS = 768;
const BATCH_SIZE = 100;
const TIMEOUT_MS = 30_000;
const MAX_ATTEMPTS = 4;

export function readKey(env = process.env) {
  if (env.GEMINI_API_KEY) return env.GEMINI_API_KEY;
  if (!env.CLAYUI_GEMINI_ENV_FILE) return null;
  try {
    const line = readFileSync(env.CLAYUI_GEMINI_ENV_FILE, 'utf8').match(/^GOOGLE_API_KEY=(.*)$/m);
    return line?.[1].trim().replace(/^["']|["']$/g, '') || null;
  } catch {
    return null;
  }
}

export const embedText = (item) => [item.title, item.description, ...(item.keywords ?? []), item.name.replace(/[-/]/g, ' ')].filter(Boolean).join('. ').slice(0, 1500);
const hash = (text) => createHash('sha256').update(`${MODEL}:${DIMENSIONS}:${text}`).digest('base64url').slice(0, 22);

const validValues = (values) => values?.length === DIMENSIONS && Array.from(values).every(Number.isFinite) && Array.from(values).some((value) => value !== 0);

function normalized(values) {
  const length = Math.hypot(...values) || 1;
  return Float32Array.from(values, (value) => value / length);
}

const toBase64 = (vector) => Buffer.from(vector.buffer, vector.byteOffset, vector.byteLength).toString('base64');
function fromBase64(text) {
  const buffer = Buffer.from(String(text), 'base64');
  if (buffer.byteLength !== DIMENSIONS * 4) return null;
  const vector = new Float32Array(buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength));
  return validValues(vector) ? vector : null;
}

async function embedBatch(texts, taskType, key) {
  for (let attempt = 1; ; attempt += 1) {
    let response;
    try {
      response = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:batchEmbedContents`, {
        method: 'POST',
        signal: AbortSignal.timeout(TIMEOUT_MS),
        headers: { 'content-type': 'application/json', 'x-goog-api-key': key },
        body: JSON.stringify({ requests: texts.map((text) => ({ model: `models/${MODEL}`, content: { parts: [{ text }] }, taskType, outputDimensionality: DIMENSIONS })) }),
      });
    } catch (error) {
      if (attempt >= MAX_ATTEMPTS) throw new Error(`Gemini: ${error.cause?.code ?? error.name}`);
      await new Promise((resolve) => setTimeout(resolve, 500 * 2 ** attempt));
      continue;
    }
    if (response.ok) {
      let embeddings;
      try {
        ({ embeddings } = await response.json());
      } catch {
        throw new Error('Gemini: niepoprawny JSON');
      }
      if (embeddings?.length !== texts.length || !embeddings.every(({ values }) => validValues(values))) throw new Error('Gemini: invalid response');
      return embeddings.map(({ values }) => normalized(values));
    }
    if ((response.status === 429 || response.status >= 500) && attempt < MAX_ATTEMPTS) {
      await new Promise((resolve) => setTimeout(resolve, 1000 * 2 ** attempt));
      continue;
    }
    throw new Error(`Gemini: HTTP ${response.status}`);
  }
}

export class VectorStore {
  constructor(directory) {
    this.file = path.join(directory, 'embeddings.json');
    try {
      this.vectors = JSON.parse(readFileSync(this.file, 'utf8'));
    } catch {
      this.vectors = {};
    }
    this.dirty = false;
  }

  // Embeds whatever is missing and returns a Map of item id -> vector.
  async ensure(items, key) {
    const texts = items.map(embedText);
    const hashes = texts.map(hash);
    const missing = [...new Map(texts.map((text, index) => [hashes[index], text]).filter(([digest]) => !fromBase64(this.vectors[digest] ?? '')))];
    for (let start = 0; start < missing.length; start += BATCH_SIZE) {
      const batch = missing.slice(start, start + BATCH_SIZE);
      const vectors = await embedBatch(batch.map(([, text]) => text), 'RETRIEVAL_DOCUMENT', key);
      batch.forEach(([digest], index) => { this.vectors[digest] = toBase64(vectors[index]); });
      this.dirty = true;
    }
    return new Map(items.map((item, index) => [item.id, fromBase64(this.vectors[hashes[index]])]));
  }

  save() {
    if (!this.dirty) return;
    mkdirSync(path.dirname(this.file), { recursive: true });
    const temporary = `${this.file}.${process.pid}.tmp`;
    writeFileSync(temporary, JSON.stringify(this.vectors));
    renameSync(temporary, this.file);
    this.dirty = false;
  }
}

export const embedQuery = async (query, key) => (await embedBatch([query], 'RETRIEVAL_QUERY', key))[0];

export function cosine(a, b) {
  if (a.length !== b.length) return 0;
  let total = 0;
  for (let index = 0; index < a.length; index += 1) total += a[index] * b[index];
  return total;
}
