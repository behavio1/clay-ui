"""Run the 120 English dev queries through a real clayui binary on a frozen cache.
Live 21st requests go to a local 404 server, so only the indexed catalog is ranked.
usage: python3 -I run.py <binary> <cache-dir> <out.json> [limit]"""
import http.server, json, os, subprocess, sys, threading

QUERIES = '/Users/Marcin_1/Documents/Behavio.one/Projects/ui-component-finder/eval/tuning/agent-queries.json'
binary, cache, out = sys.argv[1], sys.argv[2], sys.argv[3]
limit = sys.argv[4] if len(sys.argv) > 4 else '5'


class NotFound(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(404)
        self.send_header('content-length', '0')
        self.end_headers()

    def log_message(self, *args):
        pass


server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), NotFound)
threading.Thread(target=server.serve_forever, daemon=True).start()
env = {
    'PATH': os.environ['PATH'],
    'HOME': cache,
    'CLAYUI_CACHE_DIR': cache,
    'GEMINI_API_KEY': '',
    'CLAYUI_GEMINI_ENV_FILE': '',
    'CLAYUI_CDN_BASE_URL': '',
    'CLAYUI_TEST_BASE_URL': f'http://127.0.0.1:{server.server_address[1]}',
    'CLAYUI_TEST_TIMEOUT_MS': '500',
}
results = []
for q in json.load(open(QUERIES))['queries']:
    args = [binary, 'search', q['concise'], '--json', '--limit', limit]
    if q.get('source'):
        args += ['--source', q['source']]
    run = subprocess.run(args, env=env, capture_output=True, text=True, timeout=120)
    if run.returncode != 0:
        raise SystemExit(f"{q['id']}: exit {run.returncode}: {run.stderr}")
    ids = [r['id'] for r in json.loads(run.stdout)['results']]
    results.append({'id': q['id'], 'status': 'ok', 'components': ids})
json.dump({'schema': 1, 'results': results}, open(out, 'w'), indent=1)
print(out, len(results))
