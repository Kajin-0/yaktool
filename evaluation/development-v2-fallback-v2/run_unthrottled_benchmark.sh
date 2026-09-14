#!/usr/bin/env bash
set -Eeuo pipefail

# Portable launcher for the frozen, sequential Hybrid V2 fallback benchmark.
# It deliberately never installs software or downloads models.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BASE="$ROOT/evaluation/development-v2-fallback-v2"
RESULT_DIR="$BASE/results/unthrottled"
OUT="$RESULT_DIR/llama3.2-3b-production.jsonl"
META="$RESULT_DIR/llama3.2-3b-production.meta.json"
CAND="$BASE/model-candidates.jsonl"
MODEL="llama3.2:3b"
EXPECTED_CAND_SHA="4358940c99f8c66919908b43293a014add3f643131feafdc0001055a47954b7c"

cleanup() {
  ollama stop "$MODEL" >/dev/null 2>&1 || true
  if pgrep -f '/usr/local/lib/ollama/llama-server' >/dev/null 2>&1; then
    pkill -TERM -f '/usr/local/lib/ollama/llama-server' >/dev/null 2>&1 || true
    sleep 2
  fi
  if pgrep -f '/usr/local/lib/ollama/llama-server' >/dev/null 2>&1; then
    pkill -KILL -f '/usr/local/lib/ollama/llama-server' >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM HUP

die() { echo "benchmark preflight failed: $*" >&2; exit 2; }
command -v git >/dev/null || die "git is required"
command -v python3 >/dev/null || die "python3 is required"
command -v cargo >/dev/null || die "cargo is required"
command -v ollama >/dev/null || die "ollama is required; install it and retry"
command -v curl >/dev/null || die "curl is required"

cd "$ROOT"
[[ -f "$CAND" ]] || die "missing frozen candidate corpus: $CAND"
actual_sha="$(sha256sum "$CAND" | awk '{print $1}')"
[[ "$actual_sha" == "$EXPECTED_CAND_SHA" ]] || die "candidate SHA mismatch (expected $EXPECTED_CAND_SHA, got $actual_sha)"

python3 - "$CAND" <<'PY'
import json,sys
rows=[json.loads(x) for x in open(sys.argv[1], encoding='utf-8') if x.strip()]
assert len(rows)==176, len(rows)
assert len({r['id'] for r in rows})==176, 'duplicate candidate IDs'
counts={a:0 for a in ('move','search','list','find_large','clarify','unsupported')}
for r in rows: counts[r['expected']['action']]+=1
assert counts['move']==95, counts
assert counts['clarify']==30, counts
assert counts['unsupported']==18, counts
assert counts['search']+counts['list']+counts['find_large']==33, counts
print('candidate identity PASS', counts)
PY

[[ -z "$(pgrep -af '[p]ython3 .*evaluation/' || true)" ]] || die "another evaluation runner is active"
[[ -z "$(pgrep -af '/usr/local/lib/ollama/[l]lama-server' || true)" ]] || die "an Ollama model worker is already active"
curl -fsS --max-time 5 http://127.0.0.1:11434/api/version >/dev/null || die "Ollama daemon is unavailable"
ollama list | awk 'NR>1 {print $1}' | grep -Fxq "$MODEL" || die "$MODEL is not installed; install it before running"

mkdir -p "$RESULT_DIR"
if [[ -e "$META" || -e "$OUT" ]]; then
  [[ -e "$META" && -e "$OUT" ]] || die "incomplete result namespace; inspect $RESULT_DIR before resuming"
  python3 - "$META" "$actual_sha" "$(git rev-parse HEAD)" <<'PY'
import json,sys
m=json.load(open(sys.argv[1],encoding='utf-8'))
assert m.get('candidate_sha256')==sys.argv[2], 'result candidate SHA mismatch'
assert m.get('model')=='llama3.2:3b', 'result model mismatch'
assert m.get('production_head')==sys.argv[3], 'result production commit mismatch'
c=m.get('config',{})
assert (c.get('temperature'),c.get('seed'),c.get('num_ctx'),c.get('num_predict'),c.get('stream'),c.get('raw'),c.get('keep_alive'))==(0,42,2048,128,False,True,'10m'), 'result configuration mismatch'
print('resume identity PASS')
PY
fi

echo "Running Rust checks before inference..."
cargo check
cargo test
echo "Building production semantic helper..."
cargo build --manifest-path "$ROOT/evaluation/hybrid-v2/rust_helper/Cargo.toml"

HELPER="$ROOT/evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper"
python3 - "$CAND" "$HELPER" <<'PY'
import json, subprocess, sys
rows=[json.loads(x) for x in open(sys.argv[1], encoding='utf-8') if x.strip()]
p=subprocess.run([sys.argv[2]], input=''.join(json.dumps({'request':r['request']})+'\n' for r in rows), text=True, capture_output=True, check=True)
out=[json.loads(x) for x in p.stdout.splitlines() if x.strip()]
assert len(out)==len(rows), (len(out),len(rows))
assert all(x.get('route')=='needs_model' for x in out), {x.get('route') for x in out}
print('production fallback partition PASS', len(out))
PY

export YAKTOOL_RESULT_DIR="$RESULT_DIR"
export YAKTOOL_RESULT_JSONL="$OUT"
export YAKTOOL_RESULT_META="$META"
export YAKTOOL_HELPER="$HELPER"
export YAKTOOL_FIRST_TIMEOUT="300"
export YAKTOOL_WARM_TIMEOUT="180"
export YAKTOOL_INFRA_RETRIES="2"

echo "Starting sequential frozen benchmark; results checkpoint to $RESULT_DIR"
set +e
timeout --signal=TERM --kill-after=30s 21600s \
  python3 "$BASE/run_production_fallback.py"
run_rc=$?
set -e
cleanup
python3 "$BASE/score_unthrottled.py" "$RESULT_DIR" || score_rc=$?
score_rc="${score_rc:-0}"
if [[ "$run_rc" -ne 0 ]]; then exit "$run_rc"; fi
exit "$score_rc"
