#!/usr/bin/env python3
"""Classify frozen V2 prediction failures; no inference or gold mutation."""
import json,sys
from collections import Counter
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
def main(path):
 gold={r['id']:r for r in (Path(__file__).parent/'corpus.jsonl').read_text().splitlines() and [json.loads(x) for x in (Path(__file__).parent/'corpus.jsonl').read_text().splitlines()]}; rows=[json.loads(x) for x in Path(path).read_text().splitlines()]; c=Counter()
 for r in rows:
  g=gold[r['id']]; o=r.get('raw_model_output'); p=r.get('model_output',r.get('final_output')); e=g['expected']
  if not valid(p): c['invalid_modelintent']+=1
  elif p==e:c['correct']+=1
  elif p.get('action')!=e['action']:c['incorrect_action']+=1
  else:
   for f in ('source','destination','category','age_relation','age_days','size_relation','size_value','size_unit'):
    if p.get(f)!=e.get(f): c['wrong_'+f]+=1
 print(json.dumps(c,indent=2,sort_keys=True))
if __name__=='__main__': main(sys.argv[1])
