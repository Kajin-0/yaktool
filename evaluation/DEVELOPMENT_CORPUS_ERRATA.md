# Development Corpus Errata

The frozen corpus is preserved unchanged. Historical benchmark results remain tied to corpus SHA-256
`57814869bda2da8301afcaf396e7b6cd3eb9086a0f2cfb876c29cfad9ed8fe16`.

The following records contain a documented request/gold mismatch:

| Case | Request | Gold size fields |
| --- | --- | --- |
| `case_0146` | `move png files older than 30 days from home to desktop` | `size_relation=larger_than`, `size_value=500`, `size_unit=MB` |
| `case_0158` | `move text files older than 30 days from home to desktop` | `size_relation=larger_than`, `size_value=500`, `size_unit=MB` |
| `case_0170` | `move pdf files older than 30 days from home to desktop` | `size_relation=larger_than`, `size_value=500`, `size_unit=MB` |

None of these three requests explicitly contains a `larger than 500 MB` constraint.

`case_0164` is similar but differs in its age wording: its request is
`move any files today 0 days from home to desktop`, and its gold age fields are
`age_relation=today`, `age_days=0`; it also has the same unmentioned gold size
constraint (`larger_than`, `500`, `MB`). The hybrid gate treated its model proposal
separately and rejected it for an age mismatch, while the other three were handled
by the deterministic RuleInterpreter.

The frozen corpus is preserved unchanged for reproducibility. Do not silently correct
these records; any corrected benchmark must be a separately versioned corpus.

The machine-readable, in-memory-only correction record for development analysis is
`evaluation/development-corpus-corrections-v1.json`. It is keyed to the source SHA
above and does not replace the historical corpus or its benchmark results.
