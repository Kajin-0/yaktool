#!/usr/bin/env python3
import json,re,sys,hashlib
from collections import Counter
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
R=Path(__file__).resolve().parent; EXPECT={'2_load_moves':100,'3_load_moves':120,'4_load_moves':120,'5_load_moves':100,'ambiguous_adversarial_controls':40}
def norm(s): return re.sub(r'\s+',' ',s.lower().strip().rstrip('.!?'))
def load():
 rows=[json.loads(x) for x in (R/'corpus.jsonl').read_text().splitlines()]; assert len(rows)==480; assert len({x['id'] for x in rows})==480; assert len({x['request'] for x in rows})==480; assert len({norm(x['request']) for x in rows})==480; assert Counter(x['class'] for x in rows)==EXPECT
 for x in rows:
  e=x['expected']; assert valid(e); calc=sum([e['source']!='none',e['destination']!='none',e['category']!='any',e['age_relation']!='none',e['size_relation']!='none']); assert calc==x['constraint_count']
  if x['class']=='ambiguous_adversarial_controls': assert e['action'] in ('clarify','unsupported') and calc==0
  else: assert e['action']=='move' and calc==int(x['class'][0])
 print('VALIDATION PASS',len(rows),Counter(x['class'] for x in rows)); return rows
if __name__=='__main__':
 try: load()
 except Exception as e: print('VALIDATION FAIL',e,file=sys.stderr);sys.exit(1)
