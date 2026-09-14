#!/usr/bin/env python3
"""Offline E/F ablation replay from frozen model outputs.

This deliberately performs only conservative filling of default slots from
unambiguous request evidence; it never overwrites a non-default model slot.
"""
import json,re,sys
from pathlib import Path

EMPTY={'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,
       'size_relation':'none','size_value':0,'size_unit':'none'}
LOC='home|desktop|documents|downloads|pictures|archive'
def evidence(req):
 s=req.lower(); out={}
 m=re.search(r'\bfrom\s+('+LOC+r')\s+(?:to|into|in)\s+('+LOC+r')\b',s)
 if m: out.update(source=m.group(1),destination=m.group(2))
 cats=[c for c in ('pdf','png','jpeg','text') if re.search(r'\b'+c+r's?\b',s)]
 if len(cats)==1: out['category']=cats[0]
 a=re.search(r'\b(older|newer)\s+than\s+(\d+)\s+days?\b',s)
 if a: out.update(age_relation=('older_than' if a.group(1)=='older' else 'newer_than'),age_days=int(a.group(2)))
 z=re.search(r'\blarger\s+than\s+(\d+)\s+(KB|MB|GB|KiB|MiB|GiB)\b',req,re.I)
 if z: out.update(size_relation='larger_than',size_value=int(z.group(1)),size_unit=z.group(2))
 return out
def main():
 if len(sys.argv)!=3: raise SystemExit('usage: replay_slots.py input.jsonl output.jsonl')
 rows=[]
 for line in Path(sys.argv[1]).read_text().splitlines():
  r=json.loads(line); o=r.get('final_output');
  if r.get('route')=='model_fallback' and isinstance(o,dict) and o.get('action')=='move':
   e=evidence(r['request']); n=dict(o); conflict=False
   for k,v in e.items():
    default=EMPTY[k]
    if n.get(k)==default: n[k]=v
    elif n.get(k)!=v: conflict=True
   if not conflict: r['final_output']=n; r['replay']='slot_recovery'
   else: r['final_output']={'schema_version':'yaktool.model_intent.v1','action':'clarify',**EMPTY}; r['replay']='conflict_rejected'
  rows.append(r)
 Path(sys.argv[2]).write_text(''.join(json.dumps(r,separators=(',',':'))+'\n' for r in rows))
if __name__=='__main__': main()
