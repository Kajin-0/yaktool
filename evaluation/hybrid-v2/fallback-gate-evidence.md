# Hybrid V2 fallback gate evidence

This is development-only, offline evidence. No Ollama requests were made.

The old gate required `parse_move_frame(request) == Some(...)` after routing had
already required `parse_move_frame(request) == None`. The repaired gate uses the
independent `MoveEvidence` extractor instead.

## Oracle replay

The production test replays all 176 saved model candidates using their gold
ModelIntent as a synthetic model response: 95 move, 33 search/list/find_large,
30 clarify, and 18 unsupported. All 176 were accepted by the appropriate
validation path, including 95/95 fallback moves.

## Safety tests

Synthetic wrong source/destination, invented/dropped category, age, and size,
conflicting evidence, negation, and unsupported mixed-operation cases are
rejected. Age relation and value are compared symmetrically, as are size
semantics. The gate creates no filesystem paths, plans, or operations.
