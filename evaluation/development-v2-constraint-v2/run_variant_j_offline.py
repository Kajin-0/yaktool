#!/usr/bin/env python3
import json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'evaluation'))
from hybrid_v2_runtime import parse_move_frame,read_only,hard_deny,empty

def run(corpus, fallback, out):
 gold={json.loads(x)['id']:json.loads(x) for x in Path(corpus).read_text().splitlines()}; fb={json.loads(x)['id']:json.loads(x) for x in Path(fallback).read_text().splitlines()}; rows=[]
 for i,g in gold.items():
  if hard_deny(g['request']): final=empty('unsupported'); route='hard_deny'
  elif read_only(g['request']): final=read_only(g['request']); route='v2_readonly'
  elif parse_move_frame(g['request']): final=parse_move_frame(g['request']); route='deterministic_move_frame'
  else: final=fb[i].get('final_output',empty()); route='saved_model_fallback'
  rows.append({'id':i,'request':g['request'],'route':route,'model_invoked':False,'final_output':final})
 Path(out).write_text(''.join(json.dumps(x,separators=(',',':'))+'\n' for x in rows))
if __name__=='__main__':
 if len(sys.argv)!=4: raise SystemExit('usage: run_variant_j_offline.py corpus fallback output')
 run(*sys.argv[1:])
