# Hybrid V2 constraint-rich development report

This is development evidence only; no sealed V2 holdout was created.

## Corrected 480-case baseline

The current full V2 evaluator scored 33.96% exact (163/480), with 100% observed
mutation precision (143 accepted, 0 unsafe, 0 wrong-slot). Exact accuracy by
semantic load was 83% (2-load), 33.33% (3-load), 16.67% (4-load), and 0%
(5-load). Model outputs were schema/semantically valid; failures were primarily
wrong action or omitted category/age/size slots, not evidence-gate rejection.

## Ablation

Variant B (structured-slot prompt) scored 38.12% exact (183/480), 163 accepted
moves, and retained 100% observed mutation precision. Its load accuracies were
83%, 50%, 16.67%, and 0% respectively. Offline conservative slot-recovery
replays E and F produced the same result; they never overwrite conflicting
non-default slots and therefore add no authority.

Variants A/C/D from the earlier 400-case experiment remain historical evidence:
34.50%, 33.25%, and 34.75% exact respectively. A two-pass action-only (G) run
was not performed because it requires new inference and the available evidence
already shows the dominant failure is multi-slot extraction.

## Regression

The current full evaluator on the existing 600-case development corpus scored
62.67% exact, 100% observed mutation precision (180 accepted/correct moves),
81% straightforward, 83% paraphrased, 10.67% constraint-rich, and 50%
read-only exact. The evaluator includes the V2 read-only pre-parser.

## Decision

No V2 architecture was promoted. The corrected corpus shows monotonic semantic
load degradation and the target (>=50% 4-load, >=50% 5-load, >=70% mutation
recall) was not met. Mutation safety remains fail-closed and unchanged.
