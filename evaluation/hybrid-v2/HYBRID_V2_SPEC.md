# YakTool Hybrid V2 (development)

Hybrid V2 is a versioned development experiment; Hybrid V1 remains closed and unchanged.

```text
request
  ↓
Stage 0 deterministic hard-deny
  ↓
Stage 1 RuleInterpreter exact grammar
  ↓
Stage 2 deterministic read-only parsing
  ↓
Stage 3 deterministic move-frame interpretation
  ↓
Stage 4 local model fallback only if unresolved
  ↓
ModelIntent V1 validation / normalization
  ↓
MutationEvidenceGate for model-generated mutation
  ↓
trusted resolver / policy / plan / confirmation / execution core
```

Deterministic routes never invoke Ollama. The model emits only the closed
ModelIntent schema: it has no filesystem, path, plan, policy, confirmation, or
execution authority. Mutation evidence remains fail-closed and requires every
explicit mutation-relevant slot to be grounded in the request. No V1 artifact is
modified.

The current development routing of the 600-case corpus is 542/600 deterministic
(87 hard-deny, 355 RuleInterpreter, 100 read-only) and 58/600 NeedsModel
(9.67% estimated model invocation). These are development measurements, not
sealed validation.

The model prompt/configuration is versioned and hashed by the V2 runner. Constraint-rich ablations were evaluated, but none met the capability targets without a paired regression advantage; the existing V2 behavior remains the frozen safe baseline. Invalid model intents and ungrounded moves become canonical clarification/refusal outcomes.
