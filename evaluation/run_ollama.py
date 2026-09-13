#!/usr/bin/env python3
"""Run the fixed YakTool ModelIntent V1 benchmark against local Ollama only."""
import hashlib
import json
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

MODEL = "qwen35-08b-text-q5:latest"
ENDPOINT = "http://127.0.0.1:11434/api/generate"
ROOT = Path(__file__).resolve().parent
GOLD = ROOT / "model-intent-v1.jsonl"
SCHEMA = ROOT.parent / "schemas/model-intent-v1.json"
RESULTS = ROOT / "results"
PREDICTIONS = RESULTS / "qwen35-08b-text-q5-baseline.jsonl"
METADATA = RESULTS / "qwen35-08b-text-q5-baseline.meta.json"
DECODING = {"temperature": 0, "seed": 42, "num_ctx": 2048, "num_predict": 128}

PROMPT_TEMPLATE = """You are the semantic intent parser for YakTool.

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
- A request combining supported behavior with dangerous/unsupported
  behavior is unsupported.
- Never invent source, destination, category, age, or size.
- clarify and unsupported must use canonical empty slots.
- Output JSON only. No prose. No markdown.

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
<EXACT MODELINTENT SCHEMA>

User request:
<REQUEST>

JSON:
"""

def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()

def prompt_for(request, schema):
    return PROMPT_TEMPLATE.replace("<EXACT MODELINTENT SCHEMA>", schema).replace("<REQUEST>", request)

def request_once(payload):
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(ENDPOINT, data=body, headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=180) as response:
        return json.loads(response.read().decode("utf-8"))

def request_with_retry(payload, attempts=3):
    for attempt in range(attempts):
        try:
            return request_once(payload)
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError) as exc:
            if attempt + 1 == attempts:
                raise RuntimeError(f"infrastructure failure after {attempts} attempts: {exc}") from exc
            time.sleep(1.0 * (attempt + 1))

def main():
    if len(sys.argv) > 1:
        print("usage: python3 evaluation/run_ollama.py", file=sys.stderr)
        return 2
    schema = SCHEMA.read_text(encoding="utf-8")
    gold_rows = []
    with GOLD.open(encoding="utf-8") as stream:
        for line_no, line in enumerate(stream, 1):
            row = json.loads(line)
            # Deliberately retain only id/request. Gold answers never enter the payload.
            gold_rows.append({"id": row["id"], "request": row["request"]})
    if len(gold_rows) != 300 or len({row["id"] for row in gold_rows}) != 300:
        raise RuntimeError("expected 300 unique benchmark cases")
    RESULTS.mkdir(parents=True, exist_ok=True)
    prompt_hash = sha256_bytes(PROMPT_TEMPLATE.replace("<EXACT MODELINTENT SCHEMA>", schema).encode("utf-8"))
    started = time.perf_counter_ns()
    first = None
    records = []
    with PREDICTIONS.open("w", encoding="utf-8") as output:
        for index, row in enumerate(gold_rows):
            prompt = prompt_for(row["request"], schema)
            payload = {"model": MODEL, "prompt": prompt, "format": json.loads(schema), "stream": False, "think": False, "raw": True, "keep_alive": "10m", "options": DECODING}
            try:
                response = request_with_retry(payload)
                raw = response.get("response", "")
                try:
                    parsed = json.loads(raw)
                    diagnostic = {}
                except (TypeError, json.JSONDecodeError) as exc:
                    parsed = {"_invalid_json": True}
                    diagnostic = {"parse_error": str(exc), "raw_response": raw}
                record = {"id": row["id"], "model": MODEL, "output": parsed, "total_duration_ns": response.get("total_duration", 0), "load_duration_ns": response.get("load_duration", 0), "prompt_eval_count": response.get("prompt_eval_count", 0), "prompt_eval_duration_ns": response.get("prompt_eval_duration", 0), "eval_count": response.get("eval_count", 0), "eval_duration_ns": response.get("eval_duration", 0), "done_reason": response.get("done_reason", "")}
                record.update(diagnostic)
                if first is None: first = record
            except RuntimeError as exc:
                # Infrastructure failures are retried; an exhausted case is retained as invalid.
                record = {"id": row["id"], "model": MODEL, "output": {"_infrastructure_error": True}, "total_duration_ns": 0, "load_duration_ns": 0, "prompt_eval_count": 0, "prompt_eval_duration_ns": 0, "eval_count": 0, "eval_duration_ns": 0, "done_reason": "", "infrastructure_error": str(exc)}
            output.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")
            output.flush()
            print(f"{index + 1}/300 {row['id']}", file=sys.stderr)
    finished = time.perf_counter_ns()
    runner_hash = sha256_bytes(Path(__file__).read_bytes())
    metadata = {"model": MODEL, "parameters": "772.85M", "quantization": "Q5_K_M", "reported_model_size": "646 MB", "ollama_version": ollama_version(), "git_sha": git_sha(), "corpus_sha256": sha256_bytes(GOLD.read_bytes()), "schema_sha256": sha256_bytes(SCHEMA.read_bytes()), "prompt_sha256": prompt_hash, "runner_sha256": runner_hash, "timestamp": datetime.now(timezone.utc).isoformat(), "endpoint": ENDPOINT, "decoding": {**DECODING, "stream": False, "think": False, "raw": True, "keep_alive": "10m", "model": MODEL}, "num_cases": len(gold_rows), "benchmark_started_ns": started, "benchmark_finished_ns": finished, "total_wall_time_ns": finished - started, "first_record": {k: first.get(k) for k in ("id", "total_duration_ns", "load_duration_ns", "prompt_eval_count", "eval_count")} if first else None}
    METADATA.write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {PREDICTIONS} and {METADATA}")
    return 0

def ollama_version():
    import subprocess
    try:
        return subprocess.run(["ollama", "--version"], check=True, capture_output=True, text=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        return f"unavailable: {exc}"

def git_sha():
    import subprocess
    try:
        return subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT.parent, check=True, capture_output=True, text=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        return f"unavailable: {exc}"

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(1)
