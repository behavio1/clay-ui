"""Real CLI sync, one source/checkpoint at a time; artifacts only, no project installation."""
import json, os, pathlib, shutil, subprocess, tempfile, time
ROOT=pathlib.Path(__file__).resolve().parent
state_path=ROOT/'sync.json'
previous=json.loads(state_path.read_text()) if state_path.exists() else None
cache=pathlib.Path(os.environ.get('INTEGRATION_CACHE') or (previous['cache'] if previous else tempfile.mkdtemp(prefix='clayui-new-sources-',dir='/tmp'))).resolve()
state=previous or {'cache':str(cache),'sources':[]}
if state['cache']!=str(cache): raise SystemExit('Archive sync.json before starting a different cache')
(ROOT/'cache-location.txt').write_text(str(cache)+'\n')
env={**os.environ,'CLAYUI_CACHE_DIR':str(cache),'GEMINI_API_KEY':'','CLAYUI_GEMINI_ENV_FILE':'','CLAYUI_CDN_BASE_URL':''}
binary=ROOT.parents[1]/'target/debug/clayui'
old=['shadcn','magicui','aceternity','kokonut','beautifului','rareui','base-ui','radix','sonner','vaul','cmdk','number-flow']
new=[r[0] for r in json.loads((ROOT/'sources.json').read_text())]
print(json.dumps(dict(operation='configuration',cache=str(cache),sources=len(old+new),tokenPresent=bool(env.get('GITHUB_TOKEN')),embeddings=False)),flush=True)
for sid in old+new:
 if any(r['id']==sid for r in state['sources']):continue
 print(json.dumps(dict(source=sid,operation='sync',decision='start')),flush=True);start=time.monotonic()
 try:
  run=subprocess.run([str(binary),'sync','--source',sid,'--json'],env=env,capture_output=True,text=True,timeout=240)
  (ROOT/(sid+'-sync.log')).write_text(run.stderr)
  data=json.loads((cache/'index.json').read_text())['sources'].get(sid,{})
  row=dict(id=sid,seconds=round(time.monotonic()-start,3),exit=run.returncode,items=len(data.get('items',[])),error=data.get('error'))
 except Exception as e:row=dict(id=sid,seconds=round(time.monotonic()-start,3),error=str(e))
 state['sources'].append(row);state_path.write_text(json.dumps(state,indent=2)); print(json.dumps(row),flush=True)
 if sid==old[-1]:
  baseline=json.loads((cache/'index.json').read_text()); baseline['sources']={k:v for k,v in baseline['sources'].items() if k in old}
  (ROOT/'baseline-index.json').write_text(json.dumps(baseline))
  (cache/'snapshots').mkdir(exist_ok=True)
  (cache/'snapshots/baseline-index.json').write_text(json.dumps(baseline))
shutil.copyfile(cache/'index.json',ROOT/'expanded-index.json')
(cache/'snapshots').mkdir(exist_ok=True)
shutil.copyfile(cache/'index.json',cache/'snapshots/expanded-index.json')
