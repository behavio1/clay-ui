"""Read-only endpoint sample with per-source checkpoints; no credentials or code execution."""
import concurrent.futures, hashlib, json, pathlib, time, urllib.request, urllib.error
ROOT = pathlib.Path(__file__).resolve().parent
OUT = ROOT / 'evidence'
OUT.mkdir(exist_ok=True)
def fetch(source, op, url):
    target = OUT / (source + '-' + op + '.json')
    if target.exists(): return json.loads(target.read_text())
    for attempt in range(3):
        print(json.dumps(dict(source=source, operation=op, url=url, attempt=attempt+1, decision='start')),flush=True)
        start=time.monotonic()
        try:
            with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'clayui-source-verification'}),timeout=35) as r:
                raw=r.read(); status=r.status
            data=dict(status=status,seconds=round(time.monotonic()-start,3),sha256=hashlib.sha256(raw).hexdigest())
            try:data['body']=json.loads(raw)
            except ValueError:data['text']=raw.decode(errors='replace')
        except Exception as e:
            data=dict(status=getattr(e,'code',0),error=str(e),seconds=round(time.monotonic()-start,3))
        if data['status'] in [0,429,500,502,503,504] and attempt<2:
            print(json.dumps(dict(source=source,operation=op,decision='retry',delay=2**attempt)),flush=True);time.sleep(2**attempt);continue
        target.write_text(json.dumps(data)); print(json.dumps(dict(source=source,operation=op,status=data['status'],seconds=data['seconds'],decision='accept' if data['status']==200 else 'unresolved')),flush=True);return data

def probe(row):
    sid,title,index,item,license,repo,path,policy,preview=row
    idx=fetch(sid,'index',index)
    lic=fetch(sid,'license',f'https://raw.githubusercontent.com/{repo}/HEAD/{path}')
    body=idx.get('body',{}); items=body if isinstance(body,list) else body.get('items',[])
    print(json.dumps(dict(source=sid,operation='index',items=len(items),types={t:sum(i.get('type')==t for i in items) for t in sorted(set(i.get('type','') for i in items))})),flush=True)
    if policy=='RepoFree':fetch(sid,'repo-index','https://raw.githubusercontent.com/shadcnspace/shadcnspace/main/registry.json')
    candidates=[i for i in items if i.get('type') in ['registry:ui','registry:component','registry:block'] and (policy!='UiOnly' or i.get('type')=='registry:ui') and i.get('meta',{}).get('isPro')!=True]
    if candidates:
        i=candidates[0];fetch(sid,'item',item.replace('{name}',i['name']))
        if preview:fetch(sid,'preview',preview.replace('{name}',i['name']))
    (OUT/(sid+'-summary.json')).write_text(json.dumps(dict(id=sid,title=title,items=len(items),sample=candidates[0]['name'] if candidates else None,license=license,licenseVerified='Permission is hereby granted' in lic.get('text','') if license=='MIT' else 'Apache License' in lic.get('text',''))))
if __name__=='__main__':
    rows=json.loads((ROOT/'sources.json').read_text()); print(json.dumps(dict(operation='configuration',sources=len(rows),concurrency=4,attempts=3,timeout=35)),flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:list(pool.map(probe,rows))
