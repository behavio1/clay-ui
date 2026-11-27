"""Scoped icon search sanity check: hit@1 and hit@5 for expected object names (BM25, no key).
usage: python3 -I icons.py <binary> <cache-dir-with-kubai-icons-synced>"""
import json, os, subprocess, sys
binary, cache = sys.argv[1], sys.argv[2]
env = {'PATH': os.environ['PATH'], 'HOME': cache, 'CLAYUI_CACHE_DIR': cache, 'GEMINI_API_KEY': '', 'CLAYUI_GEMINI_ENV_FILE': '', 'CLAYUI_CDN_BASE_URL': ''}
h1 = h5 = 0
rows = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'icon-queries.json')))
for q, exp in rows:
    run = subprocess.run([binary, 'search', q, '--source', 'kubai-icons', '--json', '--limit', '5'], env=env, capture_output=True, text=True, timeout=60)
    ids = [r['name'] for r in json.loads(run.stdout)['results']]
    a, b = bool(ids[:1]) and ids[0] in exp, any(i in exp for i in ids)
    h1 += a; h5 += b
    print(('OK ' if a else 'ok5' if b else 'MISS'), q, ids)
print(f'hit@1 {h1}/{len(rows)}  hit@5 {h5}/{len(rows)}')
