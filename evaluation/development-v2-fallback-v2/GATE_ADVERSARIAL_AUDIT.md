# Hybrid V2 mutation gate adversarial audit

Offline only; no Ollama inference was performed.

- Gold oracle: 176/176 accepted (95/95 moves).
- Exhaustive corrupted move candidates: 2992; accepted 0; rejected 2992.
- Request-side conflict/negation/path cases: 7; accepted unsafe moves 0.
- Rust test assertions enforce both the oracle totals and zero incorrect accepts.

All corruption categories are generated deterministically for every gold move, including location permutations, category/age/size invention, omission and mismatch, swaps, and five multi-slot corruptions per move.
