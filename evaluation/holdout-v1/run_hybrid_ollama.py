#!/usr/bin/env python3
"""Frozen Hybrid V1 holdout runner. Model calls occur only on fallback routes."""
import argparse, hashlib, json, os, subprocess, sys, time, urllib.error, urllib.request
from datetime import datetime, timezone
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from evaluate_hybrid import hard_deny, evidence
from run_ollama import PROMPT_TEMPLATE

ROOT=Path(__file__).resolve().parent; REPO=ROOT.parents[1]; GOLD=ROOT/"corpus.jsonl"; SCHEMA=REPO/"schemas/model-intent-v1.json"; SPEC=REPO/"evaluation/hybrid-v1/HYBRID_V1_SPEC.md"; HELPER=ROOT/"rust_helper"; BIN=HELPER/"target/release/yaktool_holdout_helper"; RESULTS=ROOT/"results"; RESULTS.mkdir(exist_ok=True)
DECODING={"temperature":0,"seed":42,"num_ctx":2048,"num_predict":128}; EXPECTED_HOLDOUT="14db6ea9806124cf482ee1957276280ae1d8903860748e631e45e3801aa6f485"; EXPECTED_PROMPT="5e9af40c5b4dd77af9b0c768d8fb27ba9bacec888ea19c56a153827954b59532"
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def canonical(action="unsupported",source="none",destination="none",category="any",age=("none",0),size=("none",0,"none")):
    return {"schema_version":"yaktool.model_intent.v1","action":action,"source":source,"destination":destination,"category":category,"age_relation":age[0],"age_days":age[1],"size_relation":size[0],"size_value":size[1],"size_unit":size[2]}
def from_intent(i):
    def a(v): return v.get("value") if isinstance(v,dict) and v.get("kind")=="directory_alias" else "none"
    f=i.get("filters",{}); ex=f.get("extensions",[]); c="any" if not ex else "pdf" if ex==[".pdf"] else "png" if ex==[".png"] else "jpeg" if set(ex)=={".jpg",".jpeg"} else "text" if ex==[".txt"] else "any"; ag=f.get("age"); age=(ag.get("relation"),ag.get("days",0)) if ag else ("none",0); b=f.get("min_size_bytes"); size=("none",0,"none") if b is None else next((('larger_than', b // n, u) for u,n in (("GiB",1073741824),("GB",1000000000),("MiB",1048576),("MB",1000000),("KiB",1024),("KB",1000)) if b % n == 0), ("larger_than", b, "KB"))
    return canonical(i.get("action","unsupported"),a(i.get("source")),a(i.get("destination")),c,age,size)
def helper(proc,request,output=None):
    proc.stdin.write(json.dumps({"request":request,"output":output},ensure_ascii=False)+"\n"); proc.stdin.flush(); line=proc.stdout.readline();
    if not line: raise RuntimeError("Rust helper terminated")
    return json.loads(line)
def build_helper():
    if BIN.exists(): return
    subprocess.run(["cargo","build","--release"],cwd=HELPER,check=True,timeout=300)
def write_atomic(path, value):
    tmp=path.with_name(path.name+".tmp")
    tmp.write_text(json.dumps(value,indent=2)+"\n",encoding="utf-8")
    with tmp.open("rb") as f: os.fsync(f.fileno())
    os.replace(tmp,path)
def call(payload,timeout):
    req=urllib.request.Request("http://127.0.0.1:11434/api/generate",data=json.dumps(payload,ensure_ascii=False).encode(),headers={"Content-Type":"application/json"},method="POST")
    with urllib.request.urlopen(req,timeout=timeout) as r: return json.loads(r.read().decode())
def main():
    ap=argparse.ArgumentParser(); ap.add_argument("--model",required=True); args=ap.parse_args(); model=args.model
    holdout_hash=sha(GOLD); schema=SCHEMA.read_text(encoding="utf-8"); prompt_hash=hashlib.sha256(PROMPT_TEMPLATE.replace("<EXACT MODELINTENT SCHEMA>",schema).encode()).hexdigest()
    if holdout_hash!=EXPECTED_HOLDOUT: raise RuntimeError("holdout hash mismatch")
    if prompt_hash!=EXPECTED_PROMPT: raise RuntimeError("historical prompt hash mismatch")
    outpath=RESULTS/f"{model.split(':')[0].replace('/','-')}-hybrid-v1.jsonl"; metapath=outpath.with_suffix(".meta.json"); progress=outpath.with_suffix(".progress.json")
    gold=[json.loads(x) for x in GOLD.read_text(encoding="utf-8").splitlines()]; ids={r["id"] for r in gold}; done={}
    if outpath.exists():
        if not metapath.exists(): raise RuntimeError("results exist without metadata; refusing resume")
        old=json.loads(metapath.read_text()); frozen={"holdout_sha256":holdout_hash,"prompt_sha256":prompt_hash,"schema_sha256":sha(SCHEMA),"model":model,"hybrid_v1_spec_sha256":sha(SPEC),"decoding":{**DECODING,"stream":False,"think":False,"raw":True,"keep_alive":"10m","model":model}}
        for k,v in frozen.items():
            if old.get(k)!=v: raise RuntimeError(f"resume identity mismatch: {k}")
        for line in outpath.read_text(encoding="utf-8").splitlines():
            r=json.loads(line); 
            if r["id"] in done or r["id"] not in ids: raise RuntimeError("duplicate or unknown result ID")
            done[r["id"]]=r
    build_helper(); proc=subprocess.Popen([str(BIN)],cwd=HELPER,stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1); started=time.perf_counter_ns(); model_calls=sum(bool(r.get("model_invoked")) for r in done.values()); deterministic=sum(not r.get("model_invoked",False) for r in done.values()); old_meta=json.loads(metapath.read_text()) if metapath.exists() else {}; resume_count=int(old_meta.get("resume_count",0)) + (1 if done else 0); last=time.monotonic(); first_model_done=False; completed=len(done)
    frozen={"holdout_sha256":holdout_hash,"prompt_sha256":prompt_hash,"schema_sha256":sha(SCHEMA),"model":model,"hybrid_v1_spec_sha256":sha(SPEC),"decoding":{**DECODING,"stream":False,"think":False,"raw":True,"keep_alive":"10m","model":model}}
    if not metapath.exists():
        metapath.write_text(json.dumps({**frozen,"status":"running","resume_count":0,"total_case_count":1200},indent=2)+"\n",encoding="utf-8")
    mode="a" if outpath.exists() else "w"
    with outpath.open(mode,encoding="utf-8") as out:
      try:
        for g in gold:
            if g["id"] in done: continue
            if time.monotonic()-last > 180: raise RuntimeError("no-progress watchdog exceeded 180 seconds")
            req=g["request"]; deny=hard_deny(req); route=""; final=None; model_meta={"model_invoked":False}
            rr=helper(proc,req,None)
            if deny: route="hard_deny"; final=canonical(); deterministic+=1
            elif rr["rule"] in ("List","Search","FindLarge","Move"): route="rule_handled"; final=from_intent(rr["rule_intent"]); deterministic+=1
            else:
                route="model_fallback"; payload={"model":model,"prompt":PROMPT_TEMPLATE.replace("<EXACT MODELINTENT SCHEMA>",schema).replace("<REQUEST>",req),"format":json.loads(schema),"stream":False,"think":False,"raw":True,"keep_alive":"10m","options":DECODING}; model_meta["model_invoked"]=True
                response=None; err=None
                for attempt in range(3):
                    try: response=call(payload,180 if not first_model_done else 60); first_model_done=True; break
                    except (urllib.error.URLError,TimeoutError,OSError,json.JSONDecodeError) as e: err=str(e); time.sleep(attempt+1)
                if response is None:
                    final=canonical("clarify"); model_meta.update({"raw_model_output":"","model_valid":False,"gate_passed":False,"gate_reason":"infrastructure failure: "+str(err)})
                else:
                    raw=response.get("response",""); model_meta.update({"raw_model_output":raw,"total_duration_ns":response.get("total_duration",0),"load_duration_ns":response.get("load_duration",0),"prompt_eval_count":response.get("prompt_eval_count",0),"prompt_eval_duration_ns":response.get("prompt_eval_duration",0),"eval_count":response.get("eval_count",0),"eval_duration_ns":response.get("eval_duration",0),"done_reason":response.get("done_reason","")})
                    try: obj=json.loads(raw)
                    except (TypeError,json.JSONDecodeError): obj={"_invalid_json":True}
                    check=helper(proc,req,obj); valid=bool(check.get("model_valid")); model_meta["model_valid"]=valid
                    if not valid: route="model_invalid_rejected"; final=canonical("clarify"); model_meta.update({"gate_passed":False,"gate_reason":check.get("model_error", "invalid ModelIntent")})
                    elif obj.get("action")=="move":
                        ok,reason=evidence(req,obj); model_meta.update({"gate_passed":ok,"gate_reason":reason});
                        if ok: route="model_move_accepted"; final=obj
                        else: route="model_move_gate_rejected"; final=canonical("clarify")
                    else: final=obj
                model_calls+=1
            rec={"id":g["id"],"request":req,"route":route,"final_output":final,**model_meta}; out.write(json.dumps(rec,ensure_ascii=False,separators=(",",":"))+"\n"); out.flush(); os.fsync(out.fileno()); done[g["id"]]=rec; completed+=1; last=time.monotonic(); elapsed=(time.perf_counter_ns()-started)/1e9
            write_atomic(progress,{"total_cases":1200,"completed_cases":completed,"deterministic_cases":deterministic,"model_fallback_cases_encountered":model_calls,"model_calls_completed":model_calls,"last_completed_id":g["id"],"last_completion_timestamp":datetime.now(timezone.utc).isoformat(),"elapsed_seconds":elapsed})
            print(f"completed: {completed}/1200 deterministic: {deterministic} model calls completed: {model_calls} last case: {g['id']} elapsed: {elapsed:.1f}s",flush=True)
      finally: proc.terminate(); proc.wait(timeout=5)
    finished=time.perf_counter_ns(); meta={"model":model,"holdout_version":"yaktool.hybrid.holdout.v1","holdout_sha256":holdout_hash,"hybrid_v1_spec_sha256":sha(SPEC),"model_intent_schema_sha256":sha(SCHEMA),"prompt_sha256":prompt_hash,"evaluator_commit_sha":subprocess.run(["git","rev-parse","HEAD"],cwd=REPO,check=True,capture_output=True,text=True).stdout.strip(),"runner_sha256":sha(Path(__file__)),"scorer_sha256":sha(ROOT/"score_hybrid.py"),"decoding":{**DECODING,"stream":False,"think":False,"raw":True,"keep_alive":"10m","model":model},"endpoint":"http://127.0.0.1:11434/api/generate","benchmark_started_ns":started,"benchmark_finished_ns":finished,"total_case_count":1200,"resume_count":resume_count,"model_invocation_count":model_calls,"timestamp":datetime.now(timezone.utc).isoformat(),"status":"complete"}; write_atomic(metapath,meta); print(f"wrote {outpath} and {metapath}")
if __name__=="__main__":
    try: main()
    except Exception as e: print(f"error: {e}",file=sys.stderr); sys.exit(1)
