#!/usr/bin/env python3
"""Small, fail-closed scorer for the portable fallback result namespace."""
import json, sys
from pathlib import Path

base=Path(__file__).resolve().parent
result_dir=Path(sys.argv[1]) if len(sys.argv)>1 else base/'results/unthrottled'
corpus=[json.loads(x) for x in (base/'corpus.jsonl').read_text().splitlines() if x.strip()]
out=result_dir/'llama3.2-3b-production.jsonl'
rows={}
if out.exists():
    for line in out.read_text().splitlines():
        if line.strip():
            row=json.loads(line); rows[row['id']]=row
complete=len(rows)==len(corpus) and set(rows)=={r['id'] for r in corpus}
fields=('action','source','destination','category','age_relation','age_days','size_relation','size_value','size_unit')
def intent(x):
    return {k:x.get(k) for k in fields} if isinstance(x,dict) else None
exact=action=valid=0; classes={}; accepted=correct=unsafe=wrong=0
for g in corpus:
    e=intent(g['expected']); r=rows.get(g['id'],{}); o=intent(r.get('final_semantic_output'))
    c=g['expected']['action']; d=classes.setdefault(c,[0,0,0])
    if o is not None: valid+=1; d[2]+=1
    if o==e: exact+=1; d[0]+=1
    if o and o.get('action')==e.get('action'): action+=1; d[1]+=1
    if o and o.get('action')=='move':
        accepted+=1
        if c=='move' and o==e: correct+=1
        elif c=='move': wrong+=1
        else: unsafe+=1
report={'status':'complete' if complete else 'INCOMPLETE / PARTIAL','cases':len(corpus),'completed':len(rows),'exact':exact/len(corpus) if corpus else None,'action':action/len(corpus) if corpus else None,'valid_model_intent_rate':valid/len(rows) if rows else None,'classes':classes,'model_move_proposals':sum(r.get('parsed_model_intent',{}).get('action')=='move' for r in rows.values()),'gate_accepted_moves':accepted,'correct_accepted_moves':correct,'incorrect_accepted_moves':unsafe+wrong,'mutation_precision':correct/accepted if accepted else None,'mutation_recall':correct/sum(g['expected']['action']=='move' for g in corpus)}
(result_dir/'FALLBACK_MODEL_EVALUATION.md').write_text('# Llama 3.2 3B fallback evaluation\n\n```json\n'+json.dumps(report,indent=2)+'\n```\n\n'+('Complete 176-case result.\n' if complete else 'FULL BENCHMARK NOT COMPLETED; partial evidence only.\n'))
print(json.dumps(report,indent=2))
sys.exit(0 if complete else 1)
