# Production-derived fallback routing (v2)

The historical 119-row fallback set is superseded and must not be resumed. It
contained 59 `over to` move paraphrases now handled by production Rust and 10
explicit shell/command requests now hard-denied. Re-routing those rows with the
Rust production helper yields: 59 RuleInterpreter-handled, 10 hard-denied, and
50 remaining candidates.

The current 600-case development corpus is partitioned by the Rust router as:
87 hard-deny, 355 RuleInterpreter, 100 read-only, 0 move-frame (the exact rule
grammar handles these), and 58 `NeedsModel`; total 600.

The new fallback-focused development corpus has 200 gold-first cases (seed
20260916), with 142 routed to `NeedsModel` and the remainder deterministically
handled or denied. No model was called while generating these artifacts.
