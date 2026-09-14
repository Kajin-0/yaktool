#!/usr/bin/env python3
import argparse,json,re,subprocess,sys,time,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parent; REPO=ROOT.parents[1]; SCHEMA=REPO/'schemas/model-intent-v1.json'; HELPER=REPO/'evaluation/holdout-v1/rust_helper/target/release/yaktool_holdout_helper'
sys.path.insert(0,str(REPO/'evaluation')); from evaluate_hybrid import hard_deny,evidence
BASE='''You are the semantic intent parser for YakTool. Return exactly one JSON object matching the supplied JSON schema. Interpret only what the user explicitly requests. Allowed actions: list, search, find_large, move, clarify, unsupported. Allowed locations: home, desktop, documents, downloads, pictures, archive. Allowed categories: any, pdf, png, jpeg, text. Rules: missing required information or genuine ambiguity => clarify; unsupported behavior => unsupported; never invent semantics; a negated or mixed dangerous operation is unsupported; output JSON only. Canonical empty slots: source none, destination none, category any, age_relation none, age_days 0, size_relation none, size_value 0, size_unit none.'''
def empty(a='clarify'): return {'schema_version':'yaktool.model_intent.v1','action':a,'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
def ro(req):
 s=req.lower(); loc=r'(home|desktop|documents|downloads|pictures|archive)'; m=re.search(r'\b(?:in|from|for)\s+'+loc+r'\b',s) or re.search(r'\b(?:list|show|find|search)\s+'+loc+r'\b',s)
 if not m:return None
 pre=s[:m.end()]; action='search' if re.search(r'\b(find|search)\b',pre) else 'list' if re.search(r'\b(list|show)\b',pre) else None
 if not action:return None
 c=next((w for w in ('pdf','png','jpeg','text') if re.search(r'\b'+w+r'\b',s)),'any'); a=('none',0); am=re.search(r'\b(older|newer) than (\d+) days?\b',s)
 if am:a=('older_than' if am.group(1)=='older' else 'newer_than',int(am.group(2)))
 elif 'modified today' in s:a=('today',0)
 elif 'modified this week' in s:a=('this_week',0)
 z=('none',0,'none'); sm=re.search(r'larger than (\d+) (KB|MB|GB|KiB|MiB|GiB)',s,re.I)
 if sm:z=('larger_than',int(sm.group(1)),sm.group(2)); action='find_large'
 if action=='list': c='any';a=('none',0);z=('none',0,'none')
 return {**empty(action),'source':m.group(1),'category':c,'age_relation':a[0],'age_days':a[1],'size_relation':z[0],'size_value':z[1],'size_unit':z[2]}
def helper(p,req,o=None): p.stdin.write(json.dumps({'request':req,'output':o})+'\n');p.stdin.flush();return json.loads(p.stdout.readline())
def call(payload,t):
 q=urllib.request.Request('http://127.0.0.1:11434/api/chat',data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'},method='POST')
 with urllib.request.urlopen(q,timeout=t) as r:return json.loads(r.read().decode())
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--corpus',required=True);ap.add_argument('--output',required=True);a=ap.parse_args(); rows=[json.loads(x) for x in Path(a.corpus).read_text().splitlines()]; schema=SCHEMA.read_text(); old={}
 out=Path(a.output); out.parent.mkdir(parents=True,exist_ok=True)
 if out.exists(): old={json.loads(x)['id'] for x in out.read_text().splitlines()}
 p=subprocess.Popen([str(HELPER)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1); first=True
 with out.open('a') as f:
  for i,g in enumerate(rows,1):
   if g['id'] in old:continue
   req=g['request']; rec={'id':g['id'],'request':req,'model_invoked':False,'total_duration_ns':0}; rr=helper(p,req)
   if hard_deny(req): route='hard_deny'; final=empty('unsupported')
   elif rr['rule'] in ('List','Search','FindLarge','Move'): route='rule_handled'; final=empty('clarify')
   elif ro(req): route='v2_readonly'; final=ro(req)
   else:
    route='model_fallback'; payload={'model':'llama3.2:3b','messages':[{'role':'system','content':BASE+'\nJSON schema:\n'+schema},{'role':'user','content':req}],'format':json.loads(schema),'stream':False,'think':False,'keep_alive':'10m','options':{'temperature':0,'seed':42,'num_ctx':2048,'num_predict':128}}
    rsp=call(payload,180 if first else 60);first=False; raw=rsp.get('message',{}).get('content','');rec.update(model_invoked=True,raw_model_output=raw,total_duration_ns=rsp.get('total_duration',0),model_valid=False)
    try:o=json.loads(raw); chk=helper(p,req,o); rec['model_valid']=bool(chk.get('model_valid'))
    except Exception:o={'_invalid_json':True}
    final=empty() if not rec['model_valid'] else (o if o.get('action')!='move' or evidence(req,o)[0] else empty())
   rec.update(route=route,final_output=final);f.write(json.dumps(rec,separators=(',',':'))+'\n');f.flush();print(f'{i}/{len(rows)} {route}',flush=True)
 p.terminate();p.wait()
if __name__=='__main__':main()
