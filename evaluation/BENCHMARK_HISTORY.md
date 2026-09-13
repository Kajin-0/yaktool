# YakTool Benchmark History

These four runs are frozen historical evidence for the `yaktool.model_intent.v1`
development benchmark. They were generated with fixed prompts and decoding settings.

| Model | Params | Semantic valid | Exact | Move precision | Raw unsafe move FP | Warm median |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| qwen35-08b-text-q5 | 772.85M | 36.00% | 20.33% | 9.09% | 26 | 4.443 s |
| qwen2.5:3b-instruct | 3.1B | 61.67% | 37.33% | 43.09% | 40 | 2.873 s |
| llama3.2:3b | 3.2B | 54.33% | 40.33% | 55.88% | 53 | 3.083 s |
| qwen3.5:9b | 9.7B | 80.00% | 62.00% | 82.26% | 7 | 18.638 s |

## Validator-filtered qwen3.5:9b

- Raw unsafe move false positives: 7
- Validator-rejected: 7
- Accepted unsafe move false positives: 0
- Accepted moves: 55
- Correct accepted moves: 51
- Accepted move precision: 92.73%
- Accepted move recall: 61.45%
- Accepted wrong-slot moves: 4

## Rule-first hybrid experiment

- Hard-denied: 64/300
- RuleInterpreter-handled: 52/300
- Model fallback: 184/300
- No tested model achieved zero accepted mutation errors.
- All four hybrid variants had 0 unsafe-action mutation false positives, but each retained 3 accepted wrong-slot mutations.

The 300-case corpus is a **development benchmark**. Its cases have been inspected
during architecture work and it is not the final untouched production holdout.
