#!/usr/bin/env python3
"""Combine production deterministic routes with stored 176-case model output."""
import json, subprocess, sys
from pathlib import Path
BASE=Path(__file__).resolve().parent; ROOT=BASE.parents[1]
result_dir=Path(sys.argv[1]) if len(sys.argv)>1 else BASE/'results/unthrottled'
corpus=[json.loads(x) for x in (BASE/'corpus.jsonl').read_text().splitlines() if x.strip()]
live={}
out=result_dir/'llama3.2-3b-production.jsonl'
if out.exists():
    for line in out.read_text().splitlines():
        if line.strip(): live[json.loads(line)['id']]=json.loads(line)
helper=ROOT/'evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper'
p=subprocess.run([str(helper)],input=''.join(json.dumps({'request':x['request']})+'\n' for x in corpus),text=True,capture_output=True,check=True)
routes=[json.loads(x) for x in p.stdout.splitlines() if x.strip()]
assert len(routes)==len(corpus)
gold_input=''.join(json.dumps({'request':x['request'],'model_intent':x['expected']})+'\n' for x in corpus)
gp=subprocess.run([str(helper)],input=gold_input,text=True,capture_output=True,check=True)
gold=[json.loads(x) for x in gp.stdout.splitlines() if x.strip()]
assert len(gold)==len(corpus) and all(x.get('normalization_valid') for x in gold)
exact=actions=det=model=0; by={}
for g,r,e in zip(corpus,routes,gold):
    if r.get('route')=='needs_model': model+=1; actual=live.get(g['id'],{}).get('final_projection')
    else: det+=1; actual=r.get('projection')
    c=g['expected']['action']; d=by.setdefault(c,[0,0]); d[1]+=1
    if actual==e['normalized_projection']: exact+=1; d[0]+=1
    if isinstance(actual,dict) and actual.get('action')==c: actions+=1
report={'cases':len(corpus),'deterministic_cases':det,'model_cases':model,'exact':exact/len(corpus),'action':actions/len(corpus),'by_action':by,'model_invocation_rate':model/len(corpus)}
(result_dir/'WHOLE_DEVELOPMENT_SCORE.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
