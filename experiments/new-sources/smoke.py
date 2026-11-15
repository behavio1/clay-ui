import json,os,pathlib,subprocess,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parent
cache=(ROOT/'cache-location.txt').read_text().strip(); out=pathlib.Path(tempfile.mkdtemp(prefix='clayui-source-smoke-',dir='/tmp')).resolve()
env={**os.environ,'CLAYUI_CACHE_DIR':cache,'GEMINI_API_KEY':'','CLAYUI_GEMINI_ENV_FILE':''}
rows=[]
samples=[('evilcharts','bar chart','recharts-bar-chart'),('reui','data grid','data-grid'),('kibo','dropzone','dropzone'),('blocks-so','file upload','file-upload-03'),('blocks-so','sidebar','sidebar-01'),('asciicn','miami','miami'),('uiarc','button','button')]
print(json.dumps(dict(operation='configuration',samples=len(samples),output=str(out))),flush=True)
for source,query,name in samples:
 print(json.dumps(dict(source=source,item=name,operation='search/show/get',decision='start')),flush=True);start=time.monotonic()
 row=dict(id=source+'/'+name,query=query)
 for op,args in [('search',['search',query,'--source',source,'--limit','5','--json']),('show',['show',row['id'],'--json']),('get',['get',row['id'],'--json','--out',str(out)])]:
  p=subprocess.run([str(ROOT.parents[1]/'target/debug/clayui'),*args],env=env,capture_output=True,text=True,timeout=90)
  row[op+'Exit']=p.returncode
  if p.returncode:row[op+'Error']=p.stderr
  try:
   d=json.loads(p.stdout)
   if op=='search':row['searchIds']=[i['id'] for i in d.get('results',d.get('items',[]))] if isinstance(d,dict) else [i['id'] for i in d]
   elif op=='show':
    row.update({k:d.get(k) for k in ['sourceUrl','license','licenseUrl','previewUrl','previewStatus','registryDependencies','gaps','missingLocalImports']});row['sourceFiles']=len(d.get('files',[]));row['registryItems']=d.get('registryItems',[])
   elif op=='get':row['downloadedFiles']=d['files'];row['destination']=d['destination']
  except Exception as e:row[op+'ParseError']=str(e)
 row['seconds']=round(time.monotonic()-start,3);rows.append(row);(ROOT/'smoke.json').write_text(json.dumps(rows,indent=2));print(json.dumps(row),flush=True)
