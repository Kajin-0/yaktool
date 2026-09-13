#!/usr/bin/env python3
"""Run the frozen ModelIntent benchmark against qwen3.5:9b (raw completion)."""
import hashlib, json, subprocess, sys, time, urllib.error, urllib.request
from datetime import datetime, timezone
from pathlib import Path

MODEL="qwen3.5:9b"; ENDPOINT="http://127.0.0.1:11434/api/generate"; ROOT=Path(__file__).resolve().parent
GOLD=ROOT/"model-intent-v1.jsonl"; SCHEMA=ROOT.parent/"schemas/model-intent-v1.json"; RESULTS=ROOT/"results"
PREDICTIONS=RESULTS/"qwen3.5-9b-baseline.jsonl"; METADATA=RESULTS/"qwen3.5-9b-baseline.meta.json"
DECODING={"temperature":0,"seed":42,"num_ctx":2048,"num_predict":128}
PROMPT="""You are the semantic intent parser for YakTool.

Return exactly one JSON object matching the supplied JSON schema.

Interpret only what the user explicitly requests.

Allowed actions:
list, search, find_large, move, clarify, unsupported

Allowed locations:
home, desktop, documents, downloads, pictures, archive

Allowed categories:
any, pdf, png, jpeg, text

Rules:
- Missing required information or genuine ambiguity => clarify.
- Unsupported behavior => unsupported.
- Delete, shell execution, software installation, permission changes,
  service control, copying, and other unsupported computer operations
  are unsupported.
- A negated move is not a move.
- A request combining supported behavior with dangerous or unsupported
  behavior is unsupported.
- Never invent source, destination, category, age, or size.
- clarify and unsupported must use canonical empty slots.
- Output the structured result only.

Canonical empty slots:
source = none
destination = none
category = any
age_relation = none
age_days = 0
size_relation = none
size_value = 0
size_unit = none

JSON schema:
<SCHEMA>

User request:
<REQUEST>

JSON:
"""
def digest(b): return hashlib.sha256(b).hexdigest()
def request_once(payload):
    req=urllib.request.Request(ENDPOINT,data=json.dumps(payload,ensure_ascii=False).encode(),headers={"Content-Type":"application/json"},method="POST")
    with urllib.request.urlopen(req,timeout=180) as r: return json.loads(r.read().decode())
def retry(payload):
    for i in range(3):
        try: return request_once(payload)
        except (urllib.error.URLError,TimeoutError,OSError,json.JSONDecodeError) as e:
            if i==2: raise RuntimeError(f"infrastructure failure after 3 attempts: {e}") from e
            time.sleep(i+1)
def command(args):
    try: return subprocess.run(args,check=True,capture_output=True,text=True).stdout.strip()
    except (OSError,subprocess.CalledProcessError) as e: return f"unavailable: {e}"
def main():
    schema=SCHEMA.read_text(encoding="utf-8"); system=PROMPT.replace("<SCHEMA>",schema)
    gold=[{"id":r["id"],"request":r["request"]} for r in map(json.loads,GOLD.read_text(encoding="utf-8").splitlines())]
    if len(gold)!=300 or len({r["id"] for r in gold})!=300: raise RuntimeError("expected 300 unique benchmark cases")
    RESULTS.mkdir(exist_ok=True); prompt_hash=digest(system.replace("<REQUEST>","").encode()); started=time.perf_counter_ns(); first=None
    with PREDICTIONS.open("w",encoding="utf-8") as out:
        for i,row in enumerate(gold,1):
            payload={"model":MODEL,"prompt":system.replace("<REQUEST>",row["request"]),"format":json.loads(schema),"stream":False,"think":False,"raw":True,"keep_alive":"10m","options":DECODING}
            try:
                response=retry(payload); raw=response.get("response","")
                try: parsed,diag=json.loads(raw),{}
                except (TypeError,json.JSONDecodeError) as e: parsed,diag={"_invalid_json":True},{"parse_error":str(e),"raw_response":raw}
                record={"id":row["id"],"model":MODEL,"output":parsed,"total_duration_ns":response.get("total_duration",0),"load_duration_ns":response.get("load_duration",0),"prompt_eval_count":response.get("prompt_eval_count",0),"prompt_eval_duration_ns":response.get("prompt_eval_duration",0),"eval_count":response.get("eval_count",0),"eval_duration_ns":response.get("eval_duration",0),"done_reason":response.get("done_reason","")}; record.update(diag)
                if first is None: first=record
            except RuntimeError as e:
                record={"id":row["id"],"model":MODEL,"output":{"_infrastructure_error":True},"total_duration_ns":0,"load_duration_ns":0,"prompt_eval_count":0,"prompt_eval_duration_ns":0,"eval_count":0,"eval_duration_ns":0,"done_reason":"","infrastructure_error":str(e)}
            out.write(json.dumps(record,ensure_ascii=False,separators=(",",":"))+"\n"); out.flush(); print(f"{i}/300 {row['id']}",file=sys.stderr)
    finished=time.perf_counter_ns()
    meta={"model":MODEL,"architecture":"qwen35","parameters":"9.7B","quantization":"Q4_K_M","context_length":262144,"template":"{{ .Prompt }}","capabilities":["completion","vision","tools","thinking"],"ollama_version":command(["ollama","--version"]),"interface":"generate","endpoint":ENDPOINT,"git_sha":command(["git","rev-parse","HEAD"]),"corpus_sha256":digest(GOLD.read_bytes()),"schema_sha256":digest(SCHEMA.read_bytes()),"instruction_sha256":prompt_hash,"runner_sha256":digest(Path(__file__).read_bytes()),"timestamp":datetime.now(timezone.utc).isoformat(),"decoding":{**DECODING,"stream":False,"think":False,"raw":True,"keep_alive":"10m","model":MODEL},"num_cases":300,"benchmark_started_ns":started,"benchmark_finished_ns":finished,"total_wall_time_ns":finished-started,"first_record":{k:first.get(k) for k in ("id","total_duration_ns","load_duration_ns","prompt_eval_count","eval_count")} if first else None}
    METADATA.write_text(json.dumps(meta,indent=2)+"\n",encoding="utf-8"); print(f"wrote {PREDICTIONS} and {METADATA}")
if __name__=="__main__":
    try: main()
    except (OSError,RuntimeError,json.JSONDecodeError) as e: print(f"error: {e}",file=sys.stderr); sys.exit(1)
