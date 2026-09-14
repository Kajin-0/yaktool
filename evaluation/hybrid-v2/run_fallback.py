#!/usr/bin/env python3
import json,sys,urllib.request,time
from pathlib import Path
schema=Path('schemas/model-intent-v1.json').read_text()
prompt='''You are the semantic intent parser for YakTool. Return exactly one JSON object matching the supplied schema. Interpret only what the user explicitly requests. Allowed actions: list, search, find_large, move, clarify, unsupported. Allowed locations: home, desktop, documents, downloads, pictures, archive. Allowed categories: any, pdf, png, jpeg, text. Missing required information or ambiguity means clarify. Unsupported behavior means unsupported. Never invent semantics. Output JSON only. Canonical empty slots: source none, destination none, category any, age_relation none, age_days 0, size_relation none, size_value 0, size_unit none. JSON schema:\n'''+schema
def main():
 inp=Path(sys.argv[1]); out=Path(sys.argv[2]); done={}
 if out.exists(): done={json.loads(x)['id'] for x in out.read_text().splitlines()}
 rows=[json.loads(x) for x in inp.read_text().splitlines()]; first=True
 with out.open('a') as f:
  for r in rows:
   if r['id'] in done: continue
   body={'model':'llama3.2:3b','prompt':prompt+'\nUser request:\n'+r['request']+'\nJSON:','format':json.loads(schema),'stream':False,'raw':True,'keep_alive':'10m','options':{'temperature':0,'seed':42,'num_ctx':2048,'num_predict':128}}
   req=urllib.request.Request('http://127.0.0.1:11434/api/generate',data=json.dumps(body).encode(),headers={'Content-Type':'application/json'})
   try:
    with urllib.request.urlopen(req,timeout=180 if first else 60) as h: x=json.loads(h.read()); first=False
    try:o=json.loads(x.get('response',''))
    except Exception:o={'_invalid_json':True}
    rec={'id':r['id'],'request':r['request'],'model':'llama3.2:3b','output':o,'total_duration_ns':x.get('total_duration',0),'load_duration_ns':x.get('load_duration',0),'prompt_eval_count':x.get('prompt_eval_count',0),'eval_count':x.get('eval_count',0),'eval_duration_ns':x.get('eval_duration',0),'done_reason':x.get('done_reason','')}
   except Exception as e: rec={'id':r['id'],'request':r['request'],'model':'llama3.2:3b','output':{'_error':str(e)}}
   f.write(json.dumps(rec,separators=(',',':'))+'\n'); f.flush(); print(r['id'],flush=True)
if __name__=='__main__': main()
