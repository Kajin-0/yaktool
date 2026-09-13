#!/usr/bin/env python3
"""Deterministic slot/action diagnostics for the constraint development set."""
import json,sys
from pathlib import Path
FIELDS=('source','destination','category','age_relation','age_days','size_relation','size_value','size_unit')
for path in sys.argv[1:]:
 rows={json.loads(x)['id']:json.loads(x) for x in Path(path).read_text().splitlines()}; gold={json.loads(x)['id']:json.loads(x) for x in Path(__file__).with_name('corpus.jsonl').read_text().splitlines()}; counts={}
 for i,g in gold.items():
  o=rows[i].get('final_output',{}); e=g['expected'];
  if o==e: continue
  key='wrong_action' if o.get('action')!=e['action'] else 'wrong_slots'
  counts[key]=counts.get(key,0)+1
  for f in FIELDS:
   if o.get(f)!=e.get(f): counts['wrong_'+f]=counts.get('wrong_'+f,0)+1
 print(Path(path).name, json.dumps(counts,sort_keys=True))
