#!/usr/bin/env python3
"""Score the 176-candidate live universe using Rust semantic projections."""
import json, subprocess, sys
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
RESULT_DIR = Path(sys.argv[1]) if len(sys.argv) > 1 else BASE / 'results/unthrottled'
CAND = [json.loads(x) for x in (BASE / 'model-candidates.jsonl').read_text().splitlines() if x.strip()]
OUT = RESULT_DIR / 'llama3.2-3b-production.jsonl'
if len(CAND) != 176 or len({x['id'] for x in CAND}) != 176:
    raise SystemExit('candidate universe is not exactly 176 unique IDs')
pred = {}
if OUT.exists():
    for line in OUT.read_text().splitlines():
        if line.strip():
            row = json.loads(line)
            if row['id'] in pred: raise SystemExit('duplicate result ID: '+row['id'])
            pred[row['id']] = row
unexpected = set(pred) - {x['id'] for x in CAND}
if unexpected: raise SystemExit('unexpected result IDs: '+repr(sorted(unexpected)))
helper = ROOT / 'evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper'
if not helper.exists(): raise SystemExit('build Rust helper before scoring')
gold_input = ''.join(json.dumps({'request': x['request'], 'model_intent': x['expected']})+'\n' for x in CAND)
gold_proc = subprocess.run([str(helper)], input=gold_input, text=True, capture_output=True, check=True)
gold_lines = [json.loads(line) for line in gold_proc.stdout.splitlines() if line.strip()]
if len(gold_lines) != 176 or any(not x.get('normalization_valid') for x in gold_lines):
    raise SystemExit('Rust could not normalize all 176 gold ModelIntents')
gold_proj = {x['id']: line['normalized_projection'] for x, line in zip(CAND, gold_lines)}
def proj(row): return row.get('final_projection') or row.get('normalized_projection')
classes = {}
for g in CAND:
    c = g['expected']['action']; d = classes.setdefault(c, {'cases':0,'raw_mi_exact':0,'normalized_exact':0,'action':0,'valid_mi':0,'completed':0,'infra_failures':0}); d['cases'] += 1
    r = pred.get(g['id'], {}); completed = bool(r.get('response_completed')) or r.get('terminal_status') == 'semantic_response'
    if completed: d['completed'] += 1
    else: d['infra_failures'] += 1
    if r.get('model_intent_valid'): d['valid_mi'] += 1
    if isinstance(r.get('parsed_model_intent'), dict) and r['parsed_model_intent'] == g['expected']: d['raw_mi_exact'] += 1
    if proj(r) == gold_proj[g['id']]: d['normalized_exact'] += 1
    actual = r.get('parsed_model_intent')
    if isinstance(actual, dict) and actual.get('action') == c: d['action'] += 1
accepted = correct = incorrect = 0
for g in CAND:
    r = pred.get(g['id'], {}); mi = r.get('parsed_model_intent')
    if isinstance(mi, dict) and mi.get('action') == 'move':
        if r.get('gate_accepted'): accepted += 1
        if r.get('gate_accepted'):
            if proj(r) == gold_proj[g['id']] and g['expected']['action'] == 'move': correct += 1
            else: incorrect += 1
completed = sum(d['completed'] for d in classes.values()); failures = sum(d['infra_failures'] for d in classes.values())
report = {'status':'complete' if len(pred)==176 and completed+failures==176 else 'INCOMPLETE / PARTIAL','candidate_count':176,'result_records':len(pred),'semantic_responses':completed,'exhausted_infrastructure_failures':failures,'classes':classes,'json_parse_rate':sum(bool(r.get('json_parse_valid')) for r in pred.values())/completed if completed else None,'valid_model_intent_rate':sum(bool(r.get('model_intent_valid')) for r in pred.values())/completed if completed else None,'model_move_proposals':sum(isinstance(r.get('parsed_model_intent'),dict) and r['parsed_model_intent'].get('action')=='move' for r in pred.values()),'gate_accepted_moves':accepted,'gate_rejected_moves':sum(isinstance(r.get('parsed_model_intent'),dict) and r['parsed_model_intent'].get('action')=='move' and not r.get('gate_accepted') for r in pred.values()),'accepted_correct':correct,'accepted_incorrect':incorrect,'mutation_precision':correct/accepted if accepted else None,'mutation_recall':correct/95}
(RESULT_DIR / 'FALLBACK_MODEL_EVALUATION.md').write_text('# Llama 3.2 3B fallback evaluation\n\n'+('Complete 176-candidate result.\n\n' if report['status']=='complete' else 'FULL BENCHMARK NOT COMPLETED; partial evidence only.\n\n')+'```json\n'+json.dumps(report, indent=2)+'\n```\n')
print(json.dumps(report, indent=2))
sys.exit(0 if report['status']=='complete' else 1)
