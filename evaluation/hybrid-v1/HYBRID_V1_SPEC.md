# YakTool Hybrid V1

This document freezes the staged interpreter evaluated against the development
benchmark. It is an evaluation contract, not a production implementation.

```text
request
  ↓
Stage 0: deterministic hard-deny
  ↓
Stage 1: existing RuleInterpreter
  ↓ (only when rules return clarify/unsupported)
Stage 2: model fallback
  ↓
ModelIntent::validate()
  ↓
Stage 3: deterministic MutationEvidenceGate for move
  ↓
trusted YakTool core
```

## Frozen behavior

Stage 0 denies explicit delete/erase/wipe/remove, copy/duplicate/overwrite,
shell/command/sudo/chmod/chown, install/uninstall, service control, symlink
following, move cancellation, negated moves, `move nothing`, and exclusion
modifiers such as `except`, `excluding`, or `but not`. `archive` remains a valid
directory alias. A hard-denied request becomes canonical `unsupported` and does
not invoke a model.

Stage 1 has precedence. Exact `RuleInterpreter` results for `list`, `search`,
`find_large`, or `move` are final and do not invoke a model. Rule results of
`clarify` or `unsupported` proceed to fallback unless Stage 0 denied first.

Stage 2 accepts only the existing closed `ModelIntent` contract. Invalid
ModelIntent values are rejected as canonical `clarify`; valid read-only,
clarify, and unsupported intents are retained.

Stage 3 applies only to valid model `move` proposals. It requires an explicit
move/put/relocate verb, an unambiguous `from <source> to/into/in <destination>`
relationship, exact category evidence, exact age evidence (including explicit
absence), exact size evidence (including explicit absence), and no unknown or
negative mutation modifier. Failure is a canonical `clarify` refusal. No model
confidence or fuzzy matching is used.

The trusted filesystem core is not simulated by the holdout evaluator. This
specification must not be changed in response to holdout results; a changed
architecture requires a new Hybrid V2 specification and holdout.
