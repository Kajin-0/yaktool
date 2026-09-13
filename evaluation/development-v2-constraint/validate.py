#!/usr/bin/env python3
import json,re,sys
from collections import Counter
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]));from score import valid
R=Path(__file__).resolve().parent; EXPECTED={'two_constraint_moves':80,'three_constraint_moves':100,'four_constraint_moves':100,'five_constraint_moves':60,'paraphrased_multi_constraint':40,'ambiguous_adversarial_controls':20}
def n(s):return re.sub(r'\s+',' ',s.lower().strip().rstrip('.!?'))
def main():
 rows=[json.loads(x) for x in (R/'corpus.jsonl').read_text().splitlines()]; assert len(rows)==400 and len({x['id'] for x in rows})==400 and len({n(x['request']) for x in rows})==400; assert Counter(x['class'] for x in rows)==EXPECTED
 for x in rows:
  assert valid(x['expected'])
  if x['class']!='ambiguous_adversarial_controls': assert x['expected']['action']=='move' and x['expected']['source'] in x['request'].lower() and x['expected']['destination'] in x['request'].lower()
 old=set()
 for f in [Path(__file__).parents[1]/'model-intent-v1.jsonl',Path(__file__).parents[1]/'development-v2/corpus.jsonl',Path(__file__).parents[1]/'holdout-v1/corpus.jsonl']:
  old|={n(json.loads(y)['request']) for y in f.read_text().splitlines() if y.strip()}
 assert not old & {n(x['request']) for x in rows};print('CONSTRAINT CORPUS VALIDATION PASS');print(Counter(x['class'] for x in rows));print('leakage: PASS')
if __name__=='__main__':
 try:main()
 except Exception as e:print('FAIL',e,file=sys.stderr);sys.exit(1)
