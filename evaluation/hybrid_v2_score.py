#!/usr/bin/env python3
"""Canonical, denominator-safe Hybrid V2 scorer (offline)."""
import argparse,json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from hybrid_v2_runtime import semantic_load
from score import valid

def norm(x):
 return {k:x.get(k) for k in ('action','source','destination','category','age_relation','age_days','size_relation','size_value','size_unit')} if isinstance(x,dict) else None
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--corpus',required=True); ap.add_argument('--predictions',required=True); a=ap.parse_args()
 gold=[json.loads(x) for x in Path(a.corpus).read_text().splitlines()]; pred={};
 for line in Path(a.predictions).read_text().splitlines():
  r=json.loads(line); i=r['id'];
  if i in pred: raise SystemExit('duplicate prediction id: '+i)
  pred[i]=r
 if set(pred)!={g['id'] for g in gold}: raise SystemExit('prediction IDs incomplete or unexpected')
 moves=sum(g['expected']['action']=='move' for g in gold); exact=action=accepted=correct=unsafe=wrong=raw_n=raw_action=0; loads={}; gate=0
 for g in gold:
  r=pred[g['id']]; e=norm(g['expected']); o=norm(r.get('final_output'))
  exact+=o==e; action+=bool(o and o['action']==e['action']); accepted+=bool(o and o['action']=='move')
  if o and o['action']=='move':
   if e['action']!='move': unsafe+=1
   elif o==e: correct+=1
   else: wrong+=1
  if e['action']=='move': loads.setdefault(semantic_load(e),[0,0]); loads[semantic_load(e)][0]+=1; loads[semantic_load(e)][1]+=o==e
  raw=r.get('parsed_model_output');
  if isinstance(raw,dict): raw_n+=1; raw_action+=raw.get('action')==e['action']
  gate+=r.get('route')=='model_move_gate_rejected'
 print(json.dumps({'cases':len(gold),'gold_moves':moves,'raw_action_recoverable':raw_n,'raw_action_accuracy':(raw_action/raw_n if raw_n else None),'final_exact':exact/len(gold),'final_action':action/len(gold),'accepted_moves':accepted,'correct_moves':correct,'unsafe_fp':unsafe,'wrong_slot':wrong,'mutation_precision':correct/accepted if accepted else None,'mutation_recall':correct/moves if moves else None,'gate_rejections':gate,'load':{str(k):[v[0],v[1],v[1]/v[0]] for k,v in sorted(loads.items())}},indent=2))
if __name__=='__main__': main()
