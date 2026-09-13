#!/usr/bin/env python3
import argparse,hashlib,json,re,subprocess,sys,time,urllib.request
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
from evaluate_hybrid import hard_deny,evidence
ROOT=Path(__file__).resolve().parent; SCHEMA=ROOT.parents[1]/'schemas/model-intent-v1.json'; HELPER=ROOT.parents[1]/'evaluation/holdout-v1/rust_helper/target/release/yaktool_holdout_helper'
BASE='''You are the semantic intent parser for YakTool. Return exactly one JSON object matching the supplied JSON schema. Interpret only what the user explicitly requests. Allowed actions: list, search, find_large, move, clarify, unsupported. Allowed locations: home, desktop, documents, downloads, pictures, archive. Allowed categories: any, pdf, png, jpeg, text. Rules: missing required information or genuine ambiguity => clarify; unsupported behavior => unsupported; never invent semantics; a negated or mixed dangerous operation is unsupported; output JSON only. Canonical empty slots: source none, destination none, category any, age_relation none, age_days 0, size_relation none, size_value 0, size_unit none.'''
STRUCT='''Determine the requested action first. Then independently copy each explicitly stated semantic slot: source, destination, category, age, and size. Never drop a stated constraint and never infer an unstated one. If required information is missing, clarify; unsupported behavior is unsupported. Emit only final ModelIntent JSON.'''
def empty(a='clarify'):return {'schema_version':'yaktool.model_intent.v1','action':a,'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
def hints(req):
 s=req.lower(); locs='home|desktop|documents|downloads|pictures|archive'; m=re.search(r'from\s+('+locs+r')\s+(?:to|into|in)\s+('+locs+r')',s); out=[]
 if m:out += [f'source: {m.group(1)}',f'destination: {m.group(2)}']
 for c in ('pdf','png','jpeg','text'):
  if re.search(r'\b'+c+r'\b',s):out.append('category: '+c)
 a=re.search(r'\b(older|newer) than (\d+) days?',s)
 if a:out.append(f'age: {"older_than" if a.group(1)=="older" else "newer_than"} {a.group(2)} days')
 z=re.search(r'larger than (\d+) (KB|MB|GB|KiB|MiB|GiB)',req,re.I)
 if z:out.append(f'size: larger_than {z.group(1)} {z.group(2)}')
 return '\n'.join(out) if out else 'unknown'
def call(payload,t):
 q=urllib.request.Request('http://127.0.0.1:11434/api/chat',data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'},method='POST')
 with urllib.request.urlopen(q,timeout=t) as r:return json.loads(r.read().decode())
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--variant',choices=['A','B','C','D'],required=True);ap.add_argument('--output',required=True);a=ap.parse_args(); schema=SCHEMA.read_text(); rows=[json.loads(x) for x in (ROOT/'corpus.jsonl').read_text().splitlines()]; out=Path(a.output); old={}
 if out.exists(): old={json.loads(x)['id'] for x in out.read_text().splitlines()}
 h=subprocess.Popen([str(HELPER)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1); first=True
 with out.open('a') as f:
  for i,g in enumerate(rows,1):
   if g['id'] in old:continue
   req=g['request']; route=''; final=None; rec={'id':g['id'],'request':req,'model_invoked':False,'total_duration_ns':0,'eval_count':0,'eval_duration_ns':0}
   if hard_deny(req):route='hard_deny';final=empty('unsupported')
   else:
    rr=json.loads((lambda: (h.stdin.write(json.dumps({'request':req,'output':None})+'\n'),h.stdin.flush(),h.stdout.readline())[2])());
    if rr['rule'] in ('List','Search','FindLarge','Move'): route='rule_handled'; final=empty('clarify')
    else:
     route='model_fallback'; instr=BASE+('\n'+STRUCT if a.variant in ('B','D') else '')+('\nDeterministic evidence hints:\n'+hints(req) if a.variant in ('C','D') else '')+'\nJSON schema:\n'+schema; payload={'model':'llama3.2:3b','messages':[{'role':'system','content':instr},{'role':'user','content':req}],'format':json.loads(schema),'stream':False,'think':False,'keep_alive':'10m','options':{'temperature':0,'seed':42,'num_ctx':2048,'num_predict':128}}
     rsp=call(payload,180 if first else 60);first=False;raw=rsp.get('message',{}).get('content','');rec.update({'model_invoked':True,'raw_model_output':raw,'total_duration_ns':rsp.get('total_duration',0),'eval_count':rsp.get('eval_count',0),'eval_duration_ns':rsp.get('eval_duration',0)});
     try:o=json.loads(raw)
     except Exception:o={'_invalid_json':True}
     chk=json.loads((lambda: (h.stdin.write(json.dumps({'request':req,'output':o})+'\n'),h.stdin.flush(),h.stdout.readline())[2])());rec['model_valid']=bool(chk.get('model_valid')); final=o if rec['model_valid'] else empty();
     if rec['model_valid'] and o.get('action')=='move':
      ok,reason=evidence(req,o);rec['gate_passed']=ok;rec['gate_reason']=reason
      if not ok:final=empty()
   rec.update(route=route,final_output=final);f.write(json.dumps(rec,separators=(',',':'))+'\n');f.flush();print(f'{i}/{len(rows)} {route}',flush=True)
 h.terminate();h.wait()
if __name__=='__main__':main()
