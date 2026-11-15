import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
const root=path.dirname(new URL(import.meta.url).pathname);
const evalRoot=process.argv[2];
const {evaluate}=await import(pathToFileURL(path.join(evalRoot,'score.mjs')));
const read=p=>JSON.parse(readFileSync(p));
const save=(p,v)=>writeFileSync(path.join(root,p),JSON.stringify(v,null,2)+'\n');
const data=read(path.join(evalRoot,'dev.json'));data.queries=data.queries.filter(q=>q.lang==='en');
const keys=['success3at5','contextSuccess3at5','correctRejection'];
function paired(a,b){
 let seed=20261109;const rand=()=>{seed|=0;seed=seed+0x6D2B79F5|0;let t=Math.imul(seed^seed>>>15,1|seed);t=t+Math.imul(t^t>>>7,61|t)^t;return((t^t>>>14)>>>0)/4294967296};
 const old=new Map(b.map(r=>[r.id,r]));const families=[...new Set(a.map(r=>r.family))];
 const groups=families.map(f=>Object.fromEntries(keys.map(k=>{const rows=a.filter(r=>r.family===f&&r[k]!==null);return[k,[rows.reduce((s,r)=>s+r[k]-old.get(r.id)[k],0),rows.length]]})));
 const samples=Object.fromEntries(keys.map(k=>[k,[]]));
 for(let n=0;n<10000;n++){const sum=keys.map(()=>[0,0]);for(let j=0;j<families.length;j++){const group=groups[Math.floor(rand()*families.length)];keys.forEach((k,i)=>{sum[i][0]+=group[k][0];sum[i][1]+=group[k][1]});}keys.forEach((k,i)=>{if(sum[i][1])samples[k].push(sum[i][0]/sum[i][1]);});}
 return Object.fromEntries(keys.map(k=>{const a=samples[k].sort((a,b)=>a-b);return[k,{low:a[Math.floor((a.length-1)*.025)],high:a[Math.ceil((a.length-1)*.975)]}]}));
}
const reports={},summary={},pairs=new Map();const qrels=new Map(data.qrels.filter(r=>r.grade!==null).map(r=>[r.intent+'\0'+r.componentId,r]));const queries=new Map(data.queries.map(q=>[q.id,q]));
for(const file of readdirSync(root).filter(f=>f.endsWith('.results.json'))){
 const name=file.replace('.results.json',''); const input=read(path.join(root,file));
 if(input.results.length!==data.queries.length)throw Error('Incomplete run: '+name);
 console.log('score start',name);
 const report=evaluate(data,input);reports[name]=report;
 save(name+'.metrics.json',report);summary[name]={conservative:report.overall.conservative,optimistic:report.overall.optimistic,bootstrap:report.bootstrap,gates:report.gates};
 for(const r of input.results){const q=queries.get(r.id);for(const componentId of r.components.slice(0,5)){
  const k=q.intent+'\0'+componentId;if(qrels.has(k))continue;
  if(!pairs.has(k))pairs.set(k,{intent:q.intent,family:q.family,componentId,fixture:q.fixture,expected:q.expected,newSource:!['shadcn','magicui','aceternity','kokonut','beautifului','rareui','base-ui','radix','sonner','vaul','cmdk','number-flow','21st'].includes(componentId.split('/')[0]),runs:[]});
  pairs.get(k).runs.push(name);
 }}
}
for(const mode of ['bm25','gemini']){
 const a=reports['expanded-'+mode],b=reports['baseline-'+mode];if(!a||!b)continue;
 summary[mode+'-paired']={iterations:10000,seed:20261109,unit:'family',conservative:{delta:Object.fromEntries(keys.map(k=>[k,a.overall.conservative[k]-b.overall.conservative[k]])),ci95:paired(a.perQuery.conservative,b.perQuery.conservative)},optimistic:{delta:Object.fromEntries(keys.map(k=>[k,a.overall.optimistic[k]-b.overall.optimistic[k]])),ci95:paired(a.perQuery.optimistic,b.perQuery.optimistic)}};
}
save('summary.json',summary);save('pairs-to-judge.json',[...pairs.values()].sort((a,b)=>a.intent.localeCompare(b.intent)||a.componentId.localeCompare(b.componentId)));
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const files=['dev.json','score.mjs','PROTOCOL.md','README.md','tuning/agent-queries.json'];
save('input-hashes.json',Object.fromEntries(files.map(f=>[f,hash(path.join(evalRoot,f))])));
save('measurement-hashes.json',Object.fromEntries(['baseline-index.json','expanded-index.json','../../src/search.rs','../../src/embeddings.rs','retrieve.rs','score.mjs'].map(f=>[f,hash(path.join(root,f))])));
console.log(JSON.stringify(Object.fromEntries(Object.entries(summary).filter(([k])=>k.includes('baseline')||k.includes('expanded')).map(([k,v])=>[k,{lower:v.conservative.success3at5,upper:v.optimistic.success3at5,unjudged:v.conservative.unjudgedTop5Count}]))));
