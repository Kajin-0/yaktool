# Throttling-aware Llama fallback retry

**INCOMPLETE / CANARY-ONLY.** No semantic benchmark conclusion is claimed.

The fixed 12-case canary began with the first cold request. The single model
worker remained CPU-bound and produced no response by the explicit 900-second
cold-request limit. The benchmark was terminated, `llama3.2:3b` was stopped,
and no successful semantic result was written.

- Candidates: 176
- Canary attempted: 1
- Completed: 0
- Semantic retries: 0
- Infrastructure retries: 0
- Full benchmark: not completed
- New inference requests: 1 attempted, 0 completed
