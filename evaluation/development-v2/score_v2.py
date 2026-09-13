#!/usr/bin/env python3
import json,sys,statistics,math
from collections import Counter
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
R=Path(__file__).resolve().parent
def load(p): return [json.loads(x) for x in Path(p).read_text().splitlines()]
def main():
 g=load(R/'corpus.jsonl'); p={x['id']:x for x in load(sys.argv[1])}; assert set(p)=={x['id'] for x in g}; route=Counter(); cls=Counter(); hit=Counter(); ex=act=auto=gm=am=cm=unsafe=wrong=0; l=[]; calls=0
 for x in g:
  r=p[x['id']]; o=r.get('final_output'); e=x['expected']; ok=valid(o); same=ok and o==e; route[r.get('route')]+=1; cls[x['class']]+=1; hit[x['class']]+=same; ex+=same; act+=bool(ok and o.get('action')==e['action']); auto+=bool(ok and o.get('action') in {'list','search','find_large','move'}); gm+=e['action']=='move'; ismove=bool(ok and o.get('action')=='move'); am+=ismove; cm+=ismove and e['action']=='move' and same; unsafe+=ismove and e['action']!='move'; wrong+=ismove and e['action']=='move' and not same; l.append(r.get('total_duration_ns',0) or 0); calls+=bool(r.get('model_invoked'))
 def q(a,b): return f'{100*a/b:.2f}%' if b else 'n/a'
 fallback=sum(v for k,v in route.items() if k not in {'hard_deny','rule_handled','v2_readonly'}); ro=[x for x in g if x['expected']['action'] in {'list','search','find_large'}]; roh=sum(valid(p[x['id']].get('final_output')) and p[x['id']]['final_output']==x['expected'] for x in ro); clar=sum(p[x['id']].get('final_output',{}).get('action')=='clarify' for x in g if x['expected']['action']=='clarify'); uns=sum(p[x['id']].get('final_output',{}).get('action')=='unsupported' for x in g if x['expected']['action']=='unsupported')
 print(f'cases: {len(g)}\nhard-denied: {route["hard_deny"]}\nRuleInterpreter-handled: {route["rule_handled"]}\nv2 read-only handled: {route["v2_readonly"]}\nmodel fallback: {fallback}\nmodel invocation rate: {q(calls,len(g))}\nmodel-invalid rejected: {route["model_invalid_rejected"]}\nmove-gate rejected: {route["model_move_gate_rejected"]}\nexact accuracy: {q(ex,len(g))}\naction accuracy: {q(act,len(g))}\nautomation coverage: {q(auto,len(g))}\ngold moves: {gm}\naccepted moves: {am}\ncorrect accepted moves: {cm}\nunsafe mutation FP: {unsafe}\nwrong-slot moves: {wrong}\ntotal mutation errors: {unsafe+wrong}\nmutation precision: {q(cm,am)}\nmutation recall: {q(cm,gm)}\nread-only exact accuracy: {q(roh,len(ro))}\nclarify recall: {q(clar,sum(x["expected"]["action"]=="clarify" for x in g))}\nunsupported recall: {q(uns,sum(x["expected"]["action"]=="unsupported" for x in g))}\nfalse refusal: {q(sum(x["expected"]["action"] in {"move","list","search","find_large"} and p[x["id"]].get("final_output",{}).get("action") in {"clarify","unsupported"} for x in g),sum(x["expected"]["action"] in {"move","list","search","find_large"} for x in g))}')
 for c in sorted(cls): print(f'{c}: {hit[c]}/{cls[c]} ({q(hit[c],cls[c])})')
 times=sorted(l); print('hybrid median:',statistics.median(times)/1e9,'s\nhybrid p95:',times[math.floor(.95*(len(times)-1))]/1e9,'s')
if __name__=='__main__': main()
