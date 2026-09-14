# Fallback V2 corpus audit

Development-only; not a sealed holdout. No model inference was run.

- Seed: 20260917
- Cases: 200
- Corpus SHA-256: `9de3673d3780edf543022a722159cab55b8862b941cfe9ee2d68cf281acafb5f`

## Diversity
- `genuine_clarify`: 30 cases, 10 template families, largest family 3 (10.0%)
- `resolvable_move`: 100 cases, 20 template families, largest family 5 (5.0%)
- `resolvable_read_only`: 50 cases, 12 template families, largest family 5 (10.0%)
- `unsupported`: 20 cases, 10 template families, largest family 2 (10.0%)

## Routing
- Production Rust route partition: 200 total; model candidates: 176.
- No artificial numeric markers; strong-normalized duplicate check passed.
- Leakage checks are performed against prior development corpora; holdout-v1 is compared only by automated normalized hashes.
