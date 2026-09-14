#!/usr/bin/env python3
"""Evaluation-only Hybrid V2 runner for the 600-case development corpus."""
import hashlib,json,re,subprocess,sys,time,urllib.request,urllib.error
from pathlib import Path
from datetime import datetime,timezone
ROOT=Path(__file__).resolve().parent; REPO=ROOT.parents[1]; GOLD=ROOT/'corpus.jsonl'; SCHEMA=REPO/'schemas/model-intent-v1.json'; HELPER=REPO/'evaluation/holdout-v1/rust_helper/target/release/yaktool_holdout_helper'; OUT=ROOT/'results/llama3.2-hybrid-v2.jsonl'; OUT.parent.mkdir(exist_ok=True)
sys.path.insert(0,str(REPO/'evaluation'))
from evaluate_hybrid import evidence
import hybrid_v2_runtime as hv2
INSTRUCTIONS="""You are the semantic intent parser for YakTool.
Return exactly one JSON object matching the supplied JSON schema.
Interpret only what the user explicitly requests.
Allowed actions: list, search, find_large, move, clarify, unsupported
Allowed locations: home, desktop, documents, downloads, pictures, archive
Allowed categories: any, pdf, png, jpeg, text
Rules: Missing required information or genuine ambiguity => clarify. Unsupported behavior => unsupported. Delete, shell execution, software installation, permission changes, service control, copying, and other unsupported computer operations are unsupported. A negated move is not a move. A request combining supported behavior with dangerous or unsupported behavior is unsupported. Never invent source, destination, category, age, or size. clarify and unsupported must use canonical empty slots. Output the structured result only.
Canonical empty slots: source = none; destination = none; category = any; age_relation = none; age_days = 0; size_relation = none; size_value = 0; size_unit = none"""
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def empty(a='clarify'): return {'schema_version':'yaktool.model_intent.v1','action':a,'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
def model_shape(i):
 return hv2.model_intent_from_rule(i)
def helper(p,req,out=None): p.stdin.write(json.dumps({'request':req,'output':out})+'\n'); p.stdin.flush(); return json.loads(p.stdout.readline())
def read_only(req):
 s=req.lower(); loc=r'(home|desktop|documents|downloads|pictures|archive)'; lm=re.search(r'\b(?:in|from|for)\s+'+loc+r'\b',s)
 if not lm:
  lm=re.search(r'\b(?:list|show)\s+'+loc+r'\b',s)
 if not lm:
  lm=re.search(r'\b(?:find|search)\s+'+loc+r'\b',s)
 if not lm:return None
 source=lm.group(1); prefix=s[:lm.end()]; action='search' if re.search(r'\bshow\s+me\b',prefix) else 'list' if re.search(r'\b(list|show)\b',prefix) else 'search' if re.search(r'\b(find|search)\b',prefix) else None
 if not action:return None
 c='any'
 for w in ('pdf','png','jpeg','text'):
  if re.search(r'\b'+w+r'\b',s): c=w
 age=('none',0); am=re.search(r'\b(older|newer)\s+than\s+(\d+)\s+days?\b',s)
 if am: age=('older_than' if am.group(1)=='older' else 'newer_than',int(am.group(2)))
 elif re.search(r'\bmodified\s+today\b',s): age=('today',0)
 elif re.search(r'\bmodified\s+this\s+week\b',s): age=('this_week',0)
 sm=re.search(r'\blarger\s+than\s+(\d+)\s+(KB|MB|GB|KiB|MiB|GiB)\b',s,re.I); size=('larger_than',int(sm.group(1)),sm.group(2)) if sm else ('none',0,'none')
 if action=='list': c='any'; age=('none',0); size=('none',0,'none')
 if size[0]=='larger_than': action='find_large'
 return {'schema_version':'yaktool.model_intent.v1','action':action,'source':source,'destination':'none','category':c,'age_relation':age[0],'age_days':age[1],'size_relation':size[0],'size_value':size[1],'size_unit':size[2]}
def call(model,payload,timeout):
 req=urllib.request.Request('http://127.0.0.1:11434/api/chat',data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'},method='POST')
 with urllib.request.urlopen(req,timeout=timeout) as r:return json.loads(r.read().decode())
def main():
 model='llama3.2:3b'; schema=SCHEMA.read_text(); gold=[json.loads(x) for x in GOLD.read_text().splitlines()]; old={}
 if OUT.exists():
  for x in OUT.read_text().splitlines():
   r=json.loads(x); old[r['id']]=r
 p=subprocess.Popen([str(HELPER)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1)
 first=True; start=time.perf_counter_ns();
 with OUT.open('a',encoding='utf8') as f:
  for i,g in enumerate(gold,1):
   if g['id'] in old: continue
   req=g['request']; rr=helper(p,req); low=req.lower(); route=''; final=None; rec={'id':g['id'],'request':req,'model_invoked':False,'total_duration_ns':0,'load_duration_ns':0,'prompt_eval_count':0,'eval_count':0,'eval_duration_ns':0,'done_reason':''}
   if hv2.hard_deny(req): route='hard_deny'; final=empty('unsupported')
   elif rr['rule'] in ('List','Search','FindLarge','Move'): route='rule_handled'; final=model_shape(rr['normalized'] or rr['rule_intent'])
   elif hv2.read_only(req): route='v2_readonly'; final=hv2.read_only(req)
   else:
    route='model_fallback'; prompt=INSTRUCTIONS+'\nJSON schema:\n'+schema; payload={'model':model,'messages':[{'role':'system','content':prompt},{'role':'user','content':req}],'format':json.loads(schema),'stream':False,'think':False,'keep_alive':'10m','options':{'temperature':0,'seed':42,'num_ctx':2048,'num_predict':128}}
    try:
     rsp=call(model,payload,180 if first else 60); first=False; raw=rsp.get('message',{}).get('content',''); rec.update({'model_invoked':True,'model':model,'raw_model_output':raw,'total_duration_ns':rsp.get('total_duration',0),'load_duration_ns':rsp.get('load_duration',0),'prompt_eval_count':rsp.get('prompt_eval_count',0),'eval_count':rsp.get('eval_count',0),'eval_duration_ns':rsp.get('eval_duration',0),'done_reason':rsp.get('done_reason','')});
     try:o=json.loads(raw)
     except Exception:o={'_invalid_json':True}
     chk=helper(p,req,o); rec['model_valid']=bool(chk.get('model_valid'))
     if not rec['model_valid']: final=empty()
     elif o.get('action')=='move':
      ok,reason=evidence(req,o); rec['gate_passed']=ok; rec['gate_reason']=reason; final=o if ok else empty()
     else: final=o
    except Exception as e: rec.update({'model_invoked':True,'model':model,'raw_model_output':'','model_valid':False,'infrastructure_error':str(e)}); final=empty()
   rec.update({'route':route,'final_output':final}); f.write(json.dumps(rec,separators=(',',':'))+'\n'); f.flush(); print(f'{i}/{len(gold)} {g["id"]} {route}',flush=True)
 p.terminate(); p.wait(); meta={'model':model,'corpus_sha256':sha(GOLD),'schema_sha256':sha(SCHEMA),'instruction_sha256':hashlib.sha256((INSTRUCTIONS+'\nJSON schema:\n'+schema).encode()).hexdigest(),'runner_sha256':sha(Path(__file__)),'interface':'chat','timestamp':datetime.now(timezone.utc).isoformat(),'case_count':len(gold)}; (ROOT/'results/llama3.2-hybrid-v2.meta.json').write_text(json.dumps(meta,indent=2)+'\n')
if __name__=='__main__': main()
