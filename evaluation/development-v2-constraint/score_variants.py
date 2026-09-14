#!/usr/bin/env python3
import json,sys,statistics
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from hybrid_v2_runtime import semantic_load
ROOT=Path(__file__).resolve().parent
def run(path, corpus):
 gold=[json.loads(x) for x in Path(corpus).read_text().splitlines()]; pred={json.loads(x)['id']:json.loads(x) for x in Path(path).read_text().splitlines()}
 n=len(gold); exact=action=accepted=correct=unsafe=wrong=0; groups={}; drops={k:0 for k in ('category','age','size')}; inv={k:0 for k in drops}; lats=[]
 for g in gold:
  r=pred[g['id']]; o=r.get('final_output'); v=valid(o); e=g['expected']; ok=v and o==e
  exact+=ok; action+=bool(v and isinstance(o,dict) and o.get('action')==e['action'])
  if r.get('model_invoked'): lats.append(r.get('total_duration_ns',0)/1e9)
  if v and isinstance(o,dict) and o.get('action')=='move':
   accepted+=1
   if e['action']!='move': unsafe+=1
   elif o==e: correct+=1
   else: wrong+=1
  c=semantic_load(e) if e.get('action')=='move' else 0
  groups.setdefault(c,[0,0]); groups[c][0]+=1; groups[c][1]+=ok
  for k,fields in {'category':('category',),'age':('age_relation','age_days'),'size':('size_relation','size_value','size_unit')}.items():
   if any(e[f] not in (None,'none','any',0) for f in fields) and all(o.get(f) in (None,'none','any',0) for f in fields) if isinstance(o,dict) else False: drops[k]+=1
   if isinstance(o,dict) and all(e[f] in (None,'none','any',0) for f in fields) and any(o.get(f) not in (None,'none','any',0) for f in fields): inv[k]+=1
 gold_moves=sum(g.get('expected',{}).get('action')=='move' for g in gold)
 print(Path(path).name, 'exact %.2f action %.2f valid %.2f accepted %d correct %d unsafe %d wrong %d precision %.2f recall %.2f'%(exact/n*100,action/n*100,sum(valid(pred[g['id']].get('final_output')) for g in gold)/n*100,accepted,correct,unsafe,wrong,(correct/accepted*100 if accepted else 0),(correct/gold_moves*100 if gold_moves else 0)))
 print('constraint groups', {k:(v[1],v[0],v[1]/v[0]*100) for k,v in sorted(groups.items())}, 'drops',drops,'invented',inv)
 cls={}
 for g in gold:
  o=pred[g['id']].get('final_output'); cls.setdefault(g['class'],[0,0]); cls[g['class']][0]+=1; cls[g['class']][1]+=bool(valid(o) and o==g['expected'])
 print('classes', {k:(v[1],v[0],v[1]/v[0]*100) for k,v in cls.items()})
 print('latency median/p95', (statistics.median(lats) if lats else 0), (statistics.quantiles(lats,n=20)[18] if len(lats)>=20 else (max(lats) if lats else 0)))
corpus=sys.argv[1] if len(sys.argv)>1 and sys.argv[1].endswith('.jsonl') and 'corpus' in sys.argv[1] else str(ROOT/'corpus.jsonl')
paths=sys.argv[2:] if corpus!=str(ROOT/'corpus.jsonl') else sys.argv[1:]
for p in paths: run(p,corpus)
