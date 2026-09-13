#!/usr/bin/env python3
"""Run the frozen ModelIntent benchmark against qwen2.5 via Ollama chat."""
import hashlib
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

MODEL = "qwen2.5:3b-instruct"
ENDPOINT = "http://127.0.0.1:11434/api/chat"
ROOT = Path(__file__).resolve().parent
GOLD = ROOT / "model-intent-v1.jsonl"
SCHEMA = ROOT.parent / "schemas/model-intent-v1.json"
RESULTS = ROOT / "results"
PREDICTIONS = RESULTS / "qwen2.5-3b-instruct-baseline.jsonl"
METADATA = RESULTS / "qwen2.5-3b-instruct-baseline.meta.json"
DECODING = {"temperature": 0, "seed": 42, "num_ctx": 2048, "num_predict": 128}
INSTRUCTIONS = """You are the semantic intent parser for YakTool.

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
"""

def sha256(data):
    return hashlib.sha256(data).hexdigest()

def call(payload):
    body = json.dumps(payload, ensure_ascii=False).encode()
    req = urllib.request.Request(ENDPOINT, data=body, headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=180) as response:
        return json.loads(response.read().decode())

def retry(payload):
    for attempt in range(3):
        try:
            return call(payload)
        except (urllib.error.URLError, TimeoutError, OSError, json.JSONDecodeError) as exc:
            if attempt == 2:
                raise RuntimeError(f"infrastructure failure after 3 attempts: {exc}") from exc
            time.sleep(attempt + 1)

def command(args):
    try:
        return subprocess.run(args, check=True, capture_output=True, text=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        return f"unavailable: {exc}"

def main():
    schema_text = SCHEMA.read_text(encoding="utf-8")
    gold = [{"id": r["id"], "request": r["request"]} for r in map(json.loads, GOLD.read_text(encoding="utf-8").splitlines())]
    if len(gold) != 300 or len({r["id"] for r in gold}) != 300:
        raise RuntimeError("expected 300 unique benchmark cases")
    RESULTS.mkdir(exist_ok=True)
    prompt_hash = sha256((INSTRUCTIONS + "\nJSON schema:\n" + schema_text).encode())
    started = time.perf_counter_ns()
    first = None
    with PREDICTIONS.open("w", encoding="utf-8") as output:
        for index, row in enumerate(gold, 1):
            payload = {"model": MODEL, "messages": [{"role": "system", "content": INSTRUCTIONS + "\nJSON schema:\n" + schema_text}, {"role": "user", "content": row["request"]}], "format": json.loads(schema_text), "stream": False, "think": False, "keep_alive": "10m", "options": DECODING}
            try:
                response = retry(payload)
                raw = response.get("message", {}).get("content", "")
                try:
                    parsed = json.loads(raw)
                    diagnostic = {}
                except (TypeError, json.JSONDecodeError) as exc:
                    parsed = {"_invalid_json": True}
                    diagnostic = {"parse_error": str(exc), "raw_response": raw}
                record = {"id": row["id"], "model": MODEL, "output": parsed, "total_duration_ns": response.get("total_duration", 0), "load_duration_ns": response.get("load_duration", 0), "prompt_eval_count": response.get("prompt_eval_count", 0), "prompt_eval_duration_ns": response.get("prompt_eval_duration", 0), "eval_count": response.get("eval_count", 0), "eval_duration_ns": response.get("eval_duration", 0), "done_reason": response.get("done_reason", "")}
                record.update(diagnostic)
                if first is None:
                    first = record
            except RuntimeError as exc:
                record = {"id": row["id"], "model": MODEL, "output": {"_infrastructure_error": True}, "total_duration_ns": 0, "load_duration_ns": 0, "prompt_eval_count": 0, "prompt_eval_duration_ns": 0, "eval_count": 0, "eval_duration_ns": 0, "done_reason": "", "infrastructure_error": str(exc)}
            output.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")
            output.flush()
            print(f"{index}/300 {row['id']}", file=sys.stderr)
    finished = time.perf_counter_ns()
    metadata = {"model": MODEL, "architecture": "qwen2", "parameters": "3.1B", "quantization": "Q4_K_M", "context_length": 32768, "template": "Ollama chat instruction template (qwen2)", "capabilities": ["completion", "tools"], "ollama_version": command(["ollama", "--version"]), "interface": "chat", "endpoint": ENDPOINT, "git_sha": command(["git", "rev-parse", "HEAD"]), "corpus_sha256": sha256(GOLD.read_bytes()), "schema_sha256": sha256(SCHEMA.read_bytes()), "instruction_sha256": prompt_hash, "runner_sha256": sha256(Path(__file__).read_bytes()), "timestamp": datetime.now(timezone.utc).isoformat(), "decoding": {**DECODING, "stream": False, "think": False, "keep_alive": "10m", "model": MODEL}, "num_cases": 300, "benchmark_started_ns": started, "benchmark_finished_ns": finished, "total_wall_time_ns": finished - started, "first_record": {k: first.get(k) for k in ("id", "total_duration_ns", "load_duration_ns", "prompt_eval_count", "eval_count")} if first else None}
    METADATA.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {PREDICTIONS} and {METADATA}")

if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(1)
