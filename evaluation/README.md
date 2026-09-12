# Future interpreter evaluation

No model, inference server, benchmark downloader, or Python runtime is implemented in V0.1.

Future work can compare constrained local interpreters against canonical `yaktool.intent.v1` output, using supported and refused requests from the current interpretation tests. Evaluation must measure unsupported/ambiguous refusal as well as successful slot extraction. A model's output remains untrusted and must pass the same resolver, static policy, frozen-plan confirmation, and execution checks. Model evaluation never grants shell or direct filesystem mutation access.

## Frozen corpus

`model-intent-v1.jsonl` contains exactly 300 unique requests and gold `yaktool.model_intent.v1` objects. Its SHA-256 is recorded in `model-intent-v1.sha256`; Rust tests verify both the corpus structure and digest. Change the corpus and manifest together as an explicit benchmark version change.

Class counts are: straightforward read-only 50, straightforward move 50, paraphrase/case/punctuation/typos 45, multi-constraint 35, ambiguous/missing information 35, unsupported capabilities 35, negation/exclusions/corrections 25, and adversarial/mixed safe-unsafe 25.

## Prediction format and scorer

Future predictions are JSONL records with an exact corpus ID, a model name, a `ModelIntent` output, and optional inference timings:

```json
{"id":"case_0001","model":"qwen35-08b-text-q5:latest","output":{"schema_version":"yaktool.model_intent.v1","action":"move","source":"downloads","destination":"archive","category":"png","age_relation":"none","age_days":0,"size_relation":"none","size_value":0,"size_unit":"none"},"total_duration_ns":0,"load_duration_ns":0,"prompt_eval_count":0,"eval_count":0}
```

Score a complete prediction file with:

```sh
python3 evaluation/score.py evaluation/model-intent-v1.jsonl predictions.jsonl
python3 evaluation/score.py --self-test
```

The scorer rejects malformed records, missing outputs, duplicate IDs, missing IDs, or extra IDs. It reports schema-valid rate, exact and action accuracy, per-field accuracy, clarify and unsupported recall, move precision/recall, unsafe move false positives, and per-class exact accuracy. Move precision counts only exactly correct predictions among predictions whose action is `move`; unsafe move false positives count every predicted move whose expected action is not move. No release threshold is encoded.

## Intended benchmark candidates

First candidate:

```text
qwen35-08b-text-q5:latest
parameters: 772.85M
quantization: Q5_K_M
reported model size: 646 MB
configured num_ctx: 8192
template: {{ .Prompt }}
```

Later comparison candidates: `qwen2.5:3b-instruct`, `llama3.2:3b`, `qwen3.5:9b`. These names are documentation only; YakTool does not query Ollama or download any model.
