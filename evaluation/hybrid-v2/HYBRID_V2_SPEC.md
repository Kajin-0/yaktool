# YakTool Hybrid V2 (development)

Hybrid V2 is a versioned development experiment; Hybrid V1 remains closed and unchanged.

```text
request
  ↓
Stage 0 deterministic hard-deny
  ↓
Stage 1 deterministic canonical read-only grammar
  ↓
Stage 2 Llama semantic fallback
  ↓
ModelIntent::validate() / normalize()
  ↓
Stage 3 MutationEvidenceGate
  ↓
trusted YakTool core
```

V2's structural change is an evaluation-only canonical read-only pre-parser for exact `list`, `search`, and `find_large` forms, plus diagnostic classification of model failures. Mutation evidence remains fail-closed and requires explicit verb, source/destination relation, category, age, and size preservation. No model output receives filesystem authority, and no V1 artifact is modified.

The model prompt/configuration is versioned and hashed by the V2 runner. Constraint-rich ablations were evaluated, but none met the capability targets without a paired regression advantage; the existing V2 behavior remains the frozen safe baseline. Invalid model intents and ungrounded moves become canonical clarification/refusal outcomes.
