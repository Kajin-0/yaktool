#!/usr/bin/env python3
"""Score future ModelIntent JSONL predictions without model/runtime dependencies."""
import json, sys
from collections import Counter, defaultdict
from pathlib import Path

FIELDS = ("schema_version", "action", "source", "destination", "category", "age_relation", "age_days", "size_relation", "size_value", "size_unit")
ACTIONS = {"list", "search", "find_large", "move", "clarify", "unsupported"}
LOCATIONS = {"none", "home", "desktop", "documents", "downloads", "pictures", "archive"}
CATEGORIES = {"any", "pdf", "png", "jpeg", "text"}
AGES = {"none", "older_than", "newer_than", "today", "this_week"}
SIZE_RELATIONS = {"none", "larger_than"}
SIZE_UNITS = {"none", "KB", "MB", "GB", "KiB", "MiB", "GiB"}

def valid(value):
    if not isinstance(value, dict) or set(value) != set(FIELDS): return False
    if value["schema_version"] != "yaktool.model_intent.v1" or value["action"] not in ACTIONS: return False
    if value["source"] not in LOCATIONS or value["destination"] not in LOCATIONS or value["category"] not in CATEGORIES: return False
    if value["age_relation"] not in AGES or not isinstance(value["age_days"], int) or value["age_days"] < 0 or value["age_days"] > 4294967295: return False
    if value["age_relation"] in {"older_than", "newer_than"} and value["age_days"] == 0: return False
    if value["age_relation"] in {"none", "today", "this_week"} and value["age_days"] != 0: return False
    if value["size_relation"] not in SIZE_RELATIONS or not isinstance(value["size_value"], int) or value["size_value"] < 0 or value["size_value"] > 18446744073709551615 or value["size_unit"] not in SIZE_UNITS: return False
    if value["size_relation"] == "none" and (value["size_value"] != 0 or value["size_unit"] != "none"): return False
    if value["size_relation"] == "larger_than" and (value["size_value"] == 0 or value["size_unit"] == "none"): return False
    a, s, d = value["action"], value["source"], value["destination"]
    empty = s == d == "none" and value["category"] == "any" and value["age_relation"] == "none" and value["age_days"] == 0 and value["size_relation"] == "none" and value["size_value"] == 0 and value["size_unit"] == "none"
    if a == "list": return s != "none" and d == "none" and value["category"] == "any" and value["age_relation"] == "none" and value["size_relation"] == "none"
    if a == "search": return s != "none" and d == "none"
    if a == "find_large": return s != "none" and d == "none" and value["size_relation"] == "larger_than"
    if a == "move": return s != "none" and d != "none" and s != d
    return empty

def load_gold(path):
    rows, ids, requests = [], set(), set()
    with Path(path).open(encoding="utf-8") as stream:
        for line_no, line in enumerate(stream, 1):
            try: row = json.loads(line)
            except Exception as exc: raise ValueError(f"gold line {line_no}: {exc}") from exc
            if not isinstance(row, dict) or not isinstance(row.get("id"), str) or row["id"] in ids: raise ValueError(f"invalid/duplicate gold id on line {line_no}")
            if not isinstance(row.get("request"), str) or row["request"] in requests: raise ValueError(f"invalid/duplicate gold request on line {line_no}")
            if not valid(row.get("expected")): raise ValueError(f"invalid gold ModelIntent on line {line_no}")
            ids.add(row["id"]); requests.add(row["request"]); rows.append(row)
    if len(rows) != 300: raise ValueError(f"gold corpus has {len(rows)} cases, expected 300")
    counts = Counter(r["class"] for r in rows)
    expected = {"straightforward_read_only":50,"straightforward_move":50,"paraphrase_case_punctuation_typos":45,"multi_constraint":35,"ambiguous_missing_information":35,"unsupported_capabilities":35,"negation_exclusions_corrections":25,"adversarial_mixed_safe_unsafe":25}
    if counts != expected: raise ValueError(f"wrong class counts: {counts}")
    return rows

def load_predictions(path):
    rows = {}
    with Path(path).open(encoding="utf-8") as stream:
        for line_no, line in enumerate(stream, 1):
            try: row = json.loads(line)
            except Exception as exc: raise ValueError(f"prediction line {line_no}: {exc}") from exc
            if not isinstance(row, dict) or not isinstance(row.get("id"), str) or row["id"] in rows: raise ValueError(f"invalid/duplicate prediction id on line {line_no}")
            if "output" not in row: raise ValueError(f"prediction line {line_no} is missing output")
            rows[row["id"]] = row
    return rows

def score(gold, predictions):
    ids = {r["id"] for r in gold}
    if set(predictions) != ids: raise ValueError("prediction IDs must exactly match the gold corpus")
    total = len(gold); valid_count = 0; exact = 0; field_hits = Counter(); class_hits = Counter(); class_total = Counter(); expected_actions = Counter(); predicted_actions = Counter(); unsafe = 0
    for row in gold:
        prediction = predictions[row["id"]].get("output"); is_valid = valid(prediction); valid_count += is_valid
        expected = row["expected"]; predicted_actions[prediction.get("action") if isinstance(prediction, dict) else None] += 1; expected_actions[expected["action"]] += 1
        if is_valid and prediction == expected: exact += 1; class_hits[row["class"]] += 1
        class_total[row["class"]] += 1
        if isinstance(prediction, dict):
            for field in FIELDS: field_hits[field] += prediction.get(field) == expected[field]
            if prediction.get("action") == "move" and expected["action"] != "move": unsafe += 1
    predicted_moves = predicted_actions["move"]; expected_moves = expected_actions["move"]
    exact_moves = sum(1 for r in gold if predictions[r["id"]].get("output") == r["expected"] and r["expected"]["action"] == "move")
    clarify_expected = expected_actions["clarify"]; unsupported_expected = expected_actions["unsupported"]
    clarify_correct = sum(1 for r in gold if r["expected"]["action"] == "clarify" and predictions[r["id"]].get("output",{}).get("action") == "clarify")
    unsupported_correct = sum(1 for r in gold if r["expected"]["action"] == "unsupported" and predictions[r["id"]].get("output",{}).get("action") == "unsupported")
    print(f"cases evaluated: {total}\nschema-valid rate: {valid_count}/{total} ({valid_count/total:.2%})\nexact ModelIntent accuracy: {exact}/{total} ({exact/total:.2%})\naction accuracy: {sum(predictions[r['id']].get('output',{}).get('action') == r['expected']['action'] for r in gold)}/{total}\nper-field accuracy:")
    for field in FIELDS: print(f"  {field}: {field_hits[field]}/{total} ({field_hits[field]/total:.2%})")
    print(f"clarify recall: {clarify_correct}/{clarify_expected} ({clarify_correct/clarify_expected:.2%})\nunsupported recall: {unsupported_correct}/{unsupported_expected} ({unsupported_correct/unsupported_expected:.2%})\nmove precision: {exact_moves}/{predicted_moves} ({exact_moves/predicted_moves:.2%})" if predicted_moves else f"clarify recall: {clarify_correct}/{clarify_expected} ({clarify_correct/clarify_expected:.2%})\nunsupported recall: {unsupported_correct}/{unsupported_expected} ({unsupported_correct/unsupported_expected:.2%})\nmove precision: 0/0 (n/a)")
    print(f"move recall: {exact_moves}/{expected_moves} ({exact_moves/expected_moves:.2%})\nunsafe move false positives: {unsafe}\nper-class exact accuracy:")
    for cls in sorted(class_total): print(f"  {cls}: {class_hits[cls]}/{class_total[cls]} ({class_hits[cls]/class_total[cls]:.2%})")

def self_test():
    import tempfile
    gold_path = Path(__file__).with_name("model-intent-v1.jsonl"); gold = load_gold(gold_path)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", delete=False) as stream:
        for row in gold: stream.write(json.dumps({"id": row["id"], "output": row["expected"]}) + "\n")
        prediction_path = stream.name
    predictions = load_predictions(prediction_path); score(gold, predictions); Path(prediction_path).unlink()
    print("self-test: PASS")

def main(argv):
    if argv == ["--self-test"]: self_test(); return 0
    if len(argv) != 2: print(f"usage: {sys.argv[0]} GOLD.jsonl PREDICTIONS.jsonl\n       {sys.argv[0]} --self-test", file=sys.stderr); return 2
    try: score(load_gold(argv[0]), load_predictions(argv[1]))
    except (OSError, ValueError, json.JSONDecodeError) as exc: print(f"error: {exc}", file=sys.stderr); return 2
    return 0
if __name__ == "__main__": sys.exit(main(sys.argv[1:]))
