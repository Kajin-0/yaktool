#!/usr/bin/env python3
"""Synthetic scorer self-tests; uses no model and a temporary result namespace."""
import json, subprocess, tempfile
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
helper = ROOT/'evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper'
gold = [json.loads(x) for x in (BASE/'model-candidates.jsonl').read_text().splitlines() if x.strip()]
assert len(gold) == 176
p = subprocess.run([str(helper)], input=''.join(json.dumps({'request':g['request'],'model_intent':g['expected']})+'\n' for g in gold), text=True, capture_output=True, check=True)
projections = [json.loads(x)['normalized_projection'] for x in p.stdout.splitlines()]
with tempfile.TemporaryDirectory() as td:
    out = Path(td)/'llama3.2-3b-production.jsonl'
    records=[]
    for g, projection in zip(gold, projections):
        move=g['expected']['action']=='move'
        records.append({'id':g['id'],'response_completed':True,'json_parse_valid':True,'model_intent_valid':True,'parsed_model_intent':g['expected'],'final_projection':projection,'gate_accepted':move})
    out.write_text('\n'.join(json.dumps(x) for x in records)+'\n')
    score=BASE/'score_unthrottled.py'
    ok=subprocess.run(['python3',str(score),td],text=True,capture_output=True)
    assert ok.returncode==0, ok.stdout+ok.stderr
    assert json.loads(ok.stdout)['status']=='complete'
    out.write_text('\n'.join(json.dumps(x) for x in records[:-1])+'\n')
    incomplete=subprocess.run(['python3',str(score),td],text=True,capture_output=True)
    assert incomplete.returncode==1
print('scorer self-tests PASS: canonical projections, 176 completion, 175 incomplete')
