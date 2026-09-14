# Portable Hybrid V2 fallback benchmark

This is the frozen, development-only 176-case Hybrid V2 fallback benchmark. It
is intended for a Linux host with practical unthrottled CPU capacity (or
automatic Ollama GPU acceleration). It does not create or inspect a V2 holdout.

## Requirements

- Linux x86_64 (other Linux architectures may work if Rust/Ollama support them)
- `git`, `python3`, `cargo`, `curl`, and `ollama` already installed
- `llama3.2:3b` already present in `ollama list`; the launcher never downloads it
- at least 4 practical unthrottled CPU cores recommended
- at least 8 GiB available RAM; 16 GiB preferred
- local disk for the model and repository

The launcher never installs packages, uses `sudo`, changes model settings, or
starts parallel requests.

## One-command invocation

From the repository root on the benchmark host:

```bash
./evaluation/development-v2-fallback-v2/run_unthrottled_benchmark.sh
```

The launcher verifies the frozen candidate SHA and class counts, checks the
Ollama daemon/model, runs `cargo check` and `cargo test`, builds the Rust
semantic helper, then runs the fixed 12-case canary followed automatically by
the remaining candidates when the canary infrastructure gate passes. The
outer live-run ceiling is six hours; requests are sequential with 300 seconds
for the first request, 180 seconds thereafter, and at most two infrastructure
retries. Semantic retries are never performed.

## Resume and outputs

Results are isolated from the throttled VPS attempts under:

```text
evaluation/development-v2-fallback-v2/results/unthrottled/
```

The JSONL is flushed after every candidate. A matching metadata file permits
resume without rerunning successful IDs. Never mix files from another model,
candidate SHA, prompt, schema, production source, or runner version; remove or
archive an incompatible namespace and start a new one instead.

The namespace contains:

- `llama3.2-3b-production.jsonl` — durable per-case results
- `llama3.2-3b-production.meta.json` — identity, canary, hardware, and config
- `FALLBACK_MODEL_EVALUATION.md` — generated final or partial report

The runner records transport completion separately from `json_parse_valid`,
Rust `ModelIntent` deserialization/validation, normalization, gate acceptance,
and final semantic output. The scorer uses the 176 model-candidate universe
(not the 200-case source corpus), canonicalizes gold and actual semantics via
the Rust helper, and reports raw ModelIntent exactness separately from
normalized semantic exactness and mutation-gate precision/recall. A complete
fallback run has 176 terminal records; a whole fallback-focused development
score combines those 176 model cases with the 24 deterministic cases to cover
200 cases without additional inference.

Hardware metadata records an anonymized host identifier, OS/kernel,
architecture, logical CPU count, RAM/swap, virtualization/GPU hints where
available, Ollama version, and visible backend/offload information. CPU-only
and GPU-accelerated timings must not be compared as equivalent hardware.

## Cleanup and partial runs

An exit, timeout, interrupt, or failure invokes `ollama stop llama3.2:3b` and
removes a residual `llama-server` worker if necessary. Partial JSONL and
metadata remain valid evidence and are explicitly incomplete; they must not be
reported as a complete 176-case score. The launcher must be rerun only with a
matching identity manifest so completed semantic cases are skipped.

The two prior throttled-VPS attempts remain preserved in `results/` and are
operational evidence only; they are not merged with this unthrottled result
namespace.
