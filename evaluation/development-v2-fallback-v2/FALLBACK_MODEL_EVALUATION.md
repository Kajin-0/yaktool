# Llama 3.2 3B fallback evaluation

**INCOMPLETE / CANARY-ONLY — no semantic conclusion is claimed.**

The frozen production fallback runner began the deterministic 12-case canary.
The first cold request remained CPU-bound without producing a completed record
for more than five minutes. It was terminated safely and the model unloaded.

- Candidates: 176
- Completed: 0
- Semantic retries: 0
- Infrastructure retries: 0
- Full benchmark: not completed
- New Ollama requests: 1 attempted, 0 completed

No accuracy, mutation-recall, or threshold claim is made from this run.
