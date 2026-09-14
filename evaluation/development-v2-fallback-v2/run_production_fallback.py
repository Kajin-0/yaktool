#!/usr/bin/env python3
import hashlib,json,os,platform,statistics,subprocess,time,urllib.request,urllib.error
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]; BASE=ROOT/'evaluation/development-v2-fallback-v2'; CAND=BASE/'model-candidates.jsonl'; RES=Path(os.environ.get('YAKTOOL_RESULT_DIR',str(BASE/'results'))); RES.mkdir(parents=True,exist_ok=True)
OUT=Path(os.environ.get('YAKTOOL_RESULT_JSONL',str(RES/'llama3.2-3b-production.jsonl'))); META=Path(os.environ.get('YAKTOOL_RESULT_META',str(RES/'llama3.2-3b-production.meta.json'))); SCHEMA=(ROOT/'schemas/model-intent-v1.json').read_text(); MODEL='llama3.2:3b'; CONFIG={'temperature':0,'seed':42,'num_ctx':2048,'num_predict':128,'stream':False,'raw':True,'keep_alive':'10m'}
FIRST_TIMEOUT=int(os.environ.get('YAKTOOL_FIRST_TIMEOUT','300')); WARM_TIMEOUT=int(os.environ.get('YAKTOOL_WARM_TIMEOUT','180')); MAX_RETRIES=int(os.environ.get('YAKTOOL_INFRA_RETRIES','2')); HELPER=Path(os.environ.get('YAKTOOL_HELPER',str(ROOT/'evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper')))
PREFIX='You are the semantic intent parser for YakTool. Return exactly one JSON object matching the supplied schema. Interpret only what the user explicitly requests. Allowed actions: list, search, find_large, move, clarify, unsupported. Allowed locations: home, desktop, documents, downloads, pictures, archive. Allowed categories: any, pdf, png, jpeg, text. Missing required information or ambiguity means clarify. Unsupported behavior means unsupported. Never invent semantics. Output JSON only. Canonical empty slots: source none, destination none, category any, age_relation none, age_days 0, size_relation none, size_value 0, size_unit none. JSON schema:\n'
PROMPT=PREFIX+SCHEMA
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def hardware():
 def mem(key):
  try:
   for line in Path('/proc/meminfo').read_text().splitlines():
    if line.startswith(key+':'): return int(line.split()[1])
  except OSError: pass
  return None
 def cmd(args):
  try: return subprocess.check_output(args,text=True,stderr=subprocess.DEVNULL,timeout=2).strip()
  except Exception: return None
 host_hash = hashlib.sha256(platform.node().encode()).hexdigest()[:16]
 return {'host_id':host_hash,'os':platform.platform(aliased=True),'kernel':platform.release(),'architecture':platform.machine(),'logical_cpus':os.cpu_count(),'memory_total_kib':mem('MemTotal'),'swap_total_kib':mem('SwapTotal'),'virtualization':cmd(['systemd-detect-virt','--vm']) if Path('/usr/bin/systemd-detect-virt').exists() else None,'gpu_hint':cmd(['lspci']) if Path('/usr/bin/lspci').exists() else None,'ollama_version':cmd(['ollama','--version'])}
def call(req,timeout):
 body={'model':MODEL,'prompt':PROMPT+'\nUser request:\n'+req+'\nJSON:','format':json.loads(SCHEMA),**CONFIG,'options':{k:CONFIG[k] for k in ('temperature','seed','num_ctx','num_predict')}}
 b=json.dumps(body).encode(); q=urllib.request.Request('http://127.0.0.1:11434/api/generate',data=b,headers={'Content-Type':'application/json'})
 with urllib.request.urlopen(q,timeout=timeout) as h: return json.loads(h.read())
def main():
    rows = [json.loads(x) for x in CAND.read_text().splitlines() if x.strip()]
    done = {json.loads(x)['id'] for x in OUT.read_text().splitlines() if x.strip()} if OUT.exists() else set()
    canary = ([r for r in rows if r['expected']['action'] == 'move'][:6]
              + [r for r in rows if r['expected']['action'] in ('search', 'list', 'find_large')][:2]
              + [r for r in rows if r['expected']['action'] == 'clarify'][:2]
              + [r for r in rows if r['expected']['action'] == 'unsupported'][:2])
    canary_ids = {x['id'] for x in canary}
    order = canary + [r for r in rows if r['id'] not in canary_ids]
    if not META.exists():
        META.write_text(json.dumps({'result_schema_version': 'unthrottled-v1', 'model': MODEL,
            'candidate_sha256': sha(CAND), 'corpus_sha256': sha(BASE/'corpus.jsonl'), 'schema_sha256': sha(ROOT/'schemas/model-intent-v1.json'),
            'production_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'model_client_sha256': sha(ROOT/'src/model_client.rs'), 'hybrid_interpreter_sha256': sha(ROOT/'src/hybrid_interpret.rs'),
            'prompt_sha256': hashlib.sha256(PROMPT.encode()).hexdigest(), 'gate_source_sha256': sha(ROOT/'src/hybrid_interpret.rs'),
            'endpoint': 'http://127.0.0.1:11434/api/generate', 'config': CONFIG, 'candidate_count': len(rows), 'runner_sha256': sha(Path(__file__)),
            'canary_ids': [r['id'] for r in canary], 'hardware': hardware(), 'started': time.time(), 'resume_count': 0}, indent=2) + '\n')
    with OUT.open('a') as f:
        for idx, r in enumerate(order, 1):
            if r['id'] in done:
                continue
            rec = {'id': r['id'], 'class': r['class'], 'template_family': r['template_family'], 'request': r['request'],
                   'expected': r['expected'], 'model': MODEL, 'attempt_count': 0, 'attempts': [], 'model_valid': False,
                   'gate_applied': False, 'gate_accepted': False, 'final_semantic_output': None}
            for a in range(MAX_RETRIES + 1):
                rec['attempt_count'] += 1
                rec['attempts'].append({'attempt': a + 1, 'timeout_seconds': FIRST_TIMEOUT if not done and idx == 1 else WARM_TIMEOUT})
                try:
                    started_ns = time.perf_counter_ns()
                    x = call(r['request'], rec['attempts'][-1]['timeout_seconds'])
                    rec['wall_duration_ms'] = (time.perf_counter_ns() - started_ns) / 1_000_000
                    rec.update({'raw_model_output': x.get('response', ''), 'total_duration_ns': x.get('total_duration', 0),
                        'load_duration_ns': x.get('load_duration', 0), 'prompt_eval_count': x.get('prompt_eval_count', 0),
                        'eval_count': x.get('eval_count', 0), 'eval_duration_ns': x.get('eval_duration', 0),
                        'done_reason': x.get('done_reason', '')})
                    mi = json.loads(x.get('response', ''))
                    rec['parsed_model_intent'] = mi
                    rec['model_valid'] = True
                except Exception as e:
                    rec['model_error'] = str(e)
                    rec['attempts'][-1]['error'] = str(e)
                    continue
                p = subprocess.run([str(HELPER)], input=json.dumps({'request': r['request'], 'model_intent': mi}) + '\n',
                                   text=True, capture_output=True, check=False)
                out = json.loads(p.stdout.strip()) if p.stdout.strip() else {'accepted': False}
                rec['gate_applied'] = True
                rec['gate_accepted'] = bool(out.get('accepted'))
                rec['final_semantic_output'] = out.get('intent')
                rec['gate_reason'] = None if rec['gate_accepted'] else 'rejected_by_production_validation_or_evidence_gate'
                break
            f.write(json.dumps(rec, separators=(',', ':')) + '\n')
            f.flush()
            done.add(r['id'])
            print(f'completed: {len(done)}/{len(rows)}', flush=True)
            if idx == len(canary):
                canary_records = [json.loads(x) for x in OUT.read_text().splitlines() if x.strip() and json.loads(x).get('id') in canary_ids]
                canary_completed = sum(bool(x.get('model_valid')) for x in canary_records)
                canary_failed = len(canary_records) - canary_completed
                print(f'canary: {canary_completed}/{len(canary)} semantic responses, {canary_failed} infrastructure failures', flush=True)
                if canary_completed < 10 or canary_failed > 2:
                    print('canary continuation gate failed; preserving partial results', flush=True)
                    break
                durations = [x.get('wall_duration_ms') for x in canary_records if x.get('model_valid') and x.get('wall_duration_ms')]
                warm = durations[1:] if len(durations) > 1 else durations
                projected_seconds = (statistics.median(warm) / 1000.0) * (len(rows) - len(done)) * 1.25 if warm else None
                print(f'canary projected remaining seconds: {projected_seconds}', flush=True)
                if projected_seconds is not None and projected_seconds > 21600:
                    print('canary projection exceeds six-hour ceiling; preserving partial results', flush=True)
                    break
    if META.exists():
        m = json.loads(META.read_text())
        m['completed_count'] = len(done)
        m['finished'] = time.time()
        META.write_text(json.dumps(m, indent=2) + '\n')
if __name__=='__main__': main()
