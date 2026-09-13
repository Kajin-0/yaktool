#!/usr/bin/env python3
"""Paired V1/V2 analysis over identical development-v2 model proposals."""
import json,re,statistics,math,sys
from collections import Counter,defaultdict
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
from evaluate_hybrid import hard_deny,evidence
from run_v2_ollama import read_only
R=Path(__file__).resolve().parent
EMPTY=lambda a='clarify': {'schema_version':'yaktool.model_intent.v1','action':a,'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
def load(p): return [json.loads(x) for x in Path(p).read_text().splitlines()]
def classify(g,o,version):
 req=g['request']; raw=o.get('raw_model_output',''); pred=o.get('model_output',o.get('final_output'))
 if hard_deny(req): return 'hard_deny',EMPTY('unsupported')
 if version=='v2' and read_only(req): return 'v2_readonly',read_only(req)
 if o.get('route')=='rule_handled': return 'rule_handled',o['final_output']
 if not o.get('model_valid',valid(pred)): return 'model_invalid_rejected',EMPTY()
 if isinstance(pred,dict) and pred.get('action')=='move':
  ok,_=evidence(req,pred); return ('model_move_accepted',pred) if ok else ('model_move_gate_rejected',EMPTY())
 return 'model_fallback',pred if isinstance(pred,dict) else EMPTY()
def labels(g,out):
 e=g['expected']; x=out if isinstance(out,dict) else {}; labs=[]
 if x.get('action')!=e['action']: labs += ['wrong action', 'false clarify' if x.get('action')=='clarify' else 'false unsupported' if x.get('action')=='unsupported' else '']
 for f,n in [('source','source'),('destination','destination'),('category','category'),('age_relation','age relation'),('age_days','age value'),('size_relation','size relation'),('size_value','size value'),('size_unit','size unit')]:
  if x.get(f)!=e.get(f):
   labs.append('wrong '+n)
   if f in ('source','destination') and x.get(f)=='none' and e.get(f)!='none': labs.append('dropped explicit '+f)
   if f in ('category','age_relation','size_relation') and x.get(f) in (None,'none','any') and e.get(f) not in (None,'none','any'): labs.append('dropped explicit '+f.replace('_',' ')+' constraint')
   if f in ('source','destination') and x.get(f) not in (None,'none') and e.get(f)=='none': labs.append('invented '+f)
   if f in ('category','age_relation','size_relation') and x.get(f) not in (None,'none','any') and e.get(f) in (None,'none','any'): labs.append('invented '+f+' constraint')
 return [x for x in labs if x]
def main():
 src=Path(sys.argv[1]) if len(sys.argv)>1 else R/'results/llama3.2-hybrid-v2.jsonl'; gold=load(R/'corpus.jsonl'); raw=load(src); by={x['id']:x for x in raw}; assert set(by)=={x['id'] for x in gold}
 outputs={}
 for v in ('v1','v2'):
  rows=[]
  for g in gold:
   r=by[g['id']]; route,final=classify(g,r,v); rows.append({'id':g['id'],'request':g['request'],'route':route,'final_output':final,**{k:r.get(k) for k in ('model_invoked','model_valid','total_duration_ns','eval_count','eval_duration_ns')}})
  outputs[v]=rows; (R/'results'/f'llama3.2-hybrid-{v}-paired.jsonl').write_text('\n'.join(json.dumps(x,separators=(',',':')) for x in rows)+'\n')
 def stats(rows):
  s=Counter(); labs=Counter(); cls=Counter(); hit=Counter(); accepted=correct=unsafe=wrong=0
  for g,r in zip(gold,rows):
   e=g['expected']; o=r['final_output']; ok=valid(o); same=ok and o==e; s[r['route']]+=1; cls[g['class']]+=1; hit[g['class']]+=same; s['exact']+=same; s['action']+=bool(ok and o.get('action')==e['action']); s['auto']+=bool(ok and o.get('action') in {'list','search','find_large','move'}); ismove=bool(ok and o.get('action')=='move'); accepted+=ismove; correct+=ismove and e['action']=='move' and same; unsafe+=ismove and e['action']!='move'; wrong+=ismove and e['action']=='move' and not same; labs.update(labels(g,o))
  s.update(accepted=accepted,correct=correct,unsafe=unsafe,wrong=wrong,errors=unsafe+wrong); s['fallback']=sum(v for k,v in s.items() if k in {'model_fallback','model_invalid_rejected','model_move_accepted','model_move_gate_rejected'}); s['classes']=cls; s['hits']=hit; s['labels']=labs; return s
 def p(a,b): return f'{100*a/b:.2f}%' if b else 'n/a'
 for v,s in ((v,stats(rows)) for v,rows in outputs.items()):
  print(f'{v}: routes={dict((k,s[k]) for k in ("hard_deny","rule_handled","v2_readonly","model_fallback","model_invalid_rejected","model_move_gate_rejected","model_move_accepted"))}')
  print(f'  exact={p(s["exact"],600)} action={p(s["action"],600)} auto={p(s["auto"],600)} moves={s["accepted"]} correct={s["correct"]} unsafe={s["unsafe"]} wrong={s["wrong"]} precision={p(s["correct"],s["accepted"])} recall={p(s["correct"],350)}')
  for c in sorted(s['classes']): print(f'  {c}: {p(s["hits"][c],s["classes"][c])}')
  print('  taxonomy:',json.dumps(s['labels'],sort_keys=True))
 print('READONLY V2 HANDLED:',sum(1 for x in outputs['v2'] if x['route']=='v2_readonly'))
 print('DIAGNOSTIC: paired outputs written')
if __name__=='__main__': main()
