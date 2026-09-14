#!/usr/bin/env python3
import hashlib,json,subprocess,time,urllib.request,urllib.error
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]; BASE=ROOT/'evaluation/development-v2-fallback-v2'; CAND=BASE/'model-candidates.jsonl'; RES=BASE/'results'; RES.mkdir(exist_ok=True)
OUT=RES/'llama3.2-3b-production.jsonl'; META=RES/'llama3.2-3b-production.meta.json'; SCHEMA=(ROOT/'schemas/model-intent-v1.json').read_text(); MODEL='llama3.2:3b'; CONFIG={'temperature':0,'seed':42,'num_ctx':2048,'num_predict':128,'stream':False,'raw':True,'keep_alive':'10m'}
PREFIX='You are the semantic intent parser for YakTool. Return exactly one JSON object matching the supplied schema. Interpret only what the user explicitly requests. Allowed actions: list, search, find_large, move, clarify, unsupported. Allowed locations: home, desktop, documents, downloads, pictures, archive. Allowed categories: any, pdf, png, jpeg, text. Missing required information or ambiguity means clarify. Unsupported behavior means unsupported. Never invent semantics. Output JSON only. Canonical empty slots: source none, destination none, category any, age_relation none, age_days 0, size_relation none, size_value 0, size_unit none. JSON schema:\n'
PROMPT=PREFIX+SCHEMA
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def call(req,timeout):
 body={'model':MODEL,'prompt':PROMPT+'\nUser request:\n'+req+'\nJSON:','format':json.loads(SCHEMA),**CONFIG,'options':{k:CONFIG[k] for k in ('temperature','seed','num_ctx','num_predict')}}
 b=json.dumps(body).encode(); q=urllib.request.Request('http://127.0.0.1:11434/api/generate',data=b,headers={'Content-Type':'application/json'})
 with urllib.request.urlopen(q,timeout=timeout) as h: return json.loads(h.read())
def main():
 rows=[json.loads(x) for x in CAND.read_text().splitlines()]; done={json.loads(x)['id'] for x in OUT.read_text().splitlines()} if OUT.exists() else set(); attempts=0
 canary=[r for r in rows if r['expected']['action']=='move'][:6]+[r for r in rows if r['expected']['action'] in ('search','list','find_large')][:2]+[r for r in rows if r['expected']['action']=='clarify'][:2]+[r for r in rows if r['expected']['action']=='unsupported'][:2]; order=canary+[r for r in rows if r['id'] not in {x['id'] for x in canary}]
 if not META.exists(): META.write_text(json.dumps({'model':MODEL,'candidate_sha256':sha(CAND),'schema_sha256':sha(ROOT/'schemas/model-intent-v1.json'),'production_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'model_client_sha256':sha(ROOT/'src/model_client.rs'),'hybrid_interpreter_sha256':sha(ROOT/'src/hybrid_interpret.rs'),'prompt_sha256':hashlib.sha256(PROMPT.encode()).hexdigest(),'config':CONFIG,'candidate_count':len(rows),'runner_sha256':sha(Path(__file__)),'canary_ids':[r['id'] for r in canary],'started':time.time(),'resume_count':0},indent=2)+'\n')
 with OUT.open('a') as f:
  for idx,r in enumerate(order,1):
   if r['id'] in done: continue
   rec={'id':r['id'],'class':r['class'],'template_family':r['template_family'],'request':r['request'],'expected':r['expected'],'model':MODEL,'attempt_count':0,'attempts':[],'model_valid':False,'gate_applied':False,'gate_accepted':False,'final_semantic_output':None}
   for a in range(3):
    rec['attempt_count']+=1; rec['attempts'].append({'attempt':a+1,'timeout_seconds':300 if not done and idx==1 else 180})
    try:
     x=call(r['request'],rec['attempts'][-1]['timeout_seconds']); rec.update({'raw_model_output':x.get('response',''),'total_duration_ns':x.get('total_duration',0),'load_duration_ns':x.get('load_duration',0),'prompt_eval_count':x.get('prompt_eval_count',0),'eval_count':x.get('eval_count',0),'eval_duration_ns':x.get('eval_duration',0),'done_reason':x.get('done_reason','')}); mi=json.loads(x.get('response','')); rec['parsed_model_intent']=mi; rec['model_valid']=True
     except Exception as e:
      rec['model_error']=str(e); rec['attempts'][-1]['error']=str(e); continue
     # authoritative Rust validation/gate helper
     p=subprocess.run([str(ROOT/'evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper')],input=json.dumps({'request':r['request'],'model_intent':mi})+'\n',text=True,capture_output=True,check=False); out=json.loads(p.stdout.strip()) if p.stdout.strip() else {'accepted':False}; rec['gate_applied']=True; rec['gate_accepted']=bool(out.get('accepted')); rec['final_semantic_output']=out.get('intent'); rec['gate_reason']=None if rec['gate_accepted'] else 'rejected_by_production_validation_or_evidence_gate'; break
   f.write(json.dumps(rec,separators=(',',':'))+'\n'); f.flush(); done.add(r['id']); print(f'completed: {len(done)}/{len(rows)}',flush=True)
 if META.exists():
  m=json.loads(META.read_text()); m['completed_count']=len(done); m['finished']=time.time(); META.write_text(json.dumps(m,indent=2)+'\n')
if __name__=='__main__': main()
