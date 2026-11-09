// Preloaded with --import. Mode comes from CLAYUI_MOCK.
const mode = process.env.CLAYUI_MOCK;
globalThis.fetch = async (url, init = {}) => {
  if (process.env.CLAYUI_MOCK_LOG) {
    const { appendFileSync } = await import('node:fs');
    appendFileSync(process.env.CLAYUI_MOCK_LOG, `${url} ${JSON.stringify(init.headers ?? {})}\n`);
  }
  if (mode === 'timeout') throw Object.assign(new TypeError('fetch failed'), { cause: { code: 'ETIMEDOUT' } });
  if (mode === 'badjson') return new Response('<html>nope', { status: 200 });
  if (mode === 'notag') return new Response('nope', { status: 404 });
  throw new Error(`unexpected mode ${mode}`);
};
