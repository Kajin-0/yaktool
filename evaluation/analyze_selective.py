#!/usr/bin/env python3
"""Read-only selective analysis of frozen YakTool ModelIntent predictions."""
import importlib.util, json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
GOLD_PATH = ROOT / "model-intent-v1.jsonl"
MODELS = {
    "Qwen 0.8B": "qwen35-08b-text-q5-baseline.jsonl",
    "Qwen2.5 3B": "qwen2.5-3b-instruct-baseline.jsonl",
    "Llama 3.2 3B": "llama3.2-3b-baseline.jsonl",
    "Qwen3.5 9B": "qwen3.5-9b-baseline.jsonl",
}
FIELDS = ("schema_version", "action", "source", "destination", "category", "age_relation", "age_days", "size_relation", "size_value", "size_unit")

spec = importlib.util.spec_from_file_location("yaktool_score", ROOT / "score.py")
score = importlib.util.module_from_spec(spec); spec.loader.exec_module(score)

def load(path):
    return [json.loads(line) for line in Path(path).read_text(encoding="utf-8").splitlines()]

def metrics(gold, pred):
    rows = {r["id"]: r for r in pred}; accepted = []; exact = 0; actions = 0; moves = 0; correct_moves = 0; unsafe = 0; wrong_slot = 0
    for g in gold:
        p = rows[g["id"]]; o = p.get("output", {}); ok = score.valid(o)
        if ok:
            accepted.append(g)
            same = o == g["expected"]
            exact += same; actions += o.get("action") == g["expected"]["action"]
            if o.get("action") == "move":
                moves += 1
                if same and g["expected"]["action"] == "move": correct_moves += 1
                elif g["expected"]["action"] == "move": wrong_slot += 1
                else: unsafe += 1
    gold_moves = sum(g["expected"]["action"] == "move" for g in gold)
    nonmoves = [g for g in gold if g["expected"]["action"] != "move"]
    safe_nonmoves = sum(rows[g["id"]].get("output", {}).get("action") != "move" or not score.valid(rows[g["id"]].get("output", {})) for g in nonmoves)
    return {"accepted":len(accepted),"selective_exact":exact,"selective_action":actions,"moves":moves,"correct_moves":correct_moves,"move_precision":correct_moves/moves if moves else 0,"move_recall":correct_moves/gold_moves,"unsafe":unsafe,"wrong_slot":wrong_slot,"mutation_errors":unsafe+wrong_slot,"nonmove_safety":safe_nonmoves/len(nonmoves),"rejection":1-len(accepted)/len(gold),"rows":rows}

def pct(n,d): return f"{100*n/d:.2f}%" if d else "n/a"
def intent(o): return json.dumps(o, ensure_ascii=False, sort_keys=True)
def invalid_reason(v):
    if not isinstance(v, dict) or set(v) != set(score.FIELDS): return "wrong field set"
    if v.get("schema_version") != "yaktool.model_intent.v1": return "schema_version invalid"
    if v.get("action") not in score.ACTIONS: return "action enum invalid"
    if v.get("source") not in score.LOCATIONS or v.get("destination") not in score.LOCATIONS: return "location enum invalid"
    if v.get("category") not in score.CATEGORIES: return "category enum invalid"
    if v.get("age_relation") not in score.AGES or not isinstance(v.get("age_days"), int) or v.get("age_days", -1) < 0: return "age fields invalid"
    if v["age_relation"] in {"older_than", "newer_than"} and v["age_days"] == 0: return "older/newer age_days must be positive"
    if v["age_relation"] in {"none", "today", "this_week"} and v["age_days"] != 0: return "non-count age relation requires age_days=0"
    if v.get("size_relation") not in score.SIZE_RELATIONS or not isinstance(v.get("size_value"), int) or v.get("size_value", -1) < 0 or v.get("size_unit") not in score.SIZE_UNITS: return "size fields invalid"
    if v["size_relation"] == "none" and (v["size_value"] != 0 or v["size_unit"] != "none"): return "size none requires zero/none slots"
    if v["size_relation"] == "larger_than" and (v["size_value"] == 0 or v["size_unit"] == "none"): return "larger_than requires positive value and unit"
    a,s,d=v["action"],v["source"],v["destination"]
    empty=s==d=="none" and v["category"]=="any" and v["age_relation"]=="none" and v["age_days"]==0 and v["size_relation"]=="none" and v["size_value"]==0 and v["size_unit"]=="none"
    if a=="list" and not (s!="none" and d=="none" and v["category"]=="any" and v["age_relation"]=="none" and v["size_relation"]=="none"): return "list semantic requirements"
    if a=="search" and not (s!="none" and d=="none"): return "search requires source and no destination"
    if a=="find_large" and not (s!="none" and d=="none" and v["size_relation"]=="larger_than"): return "find_large requires source, no destination, larger_than"
    if a=="move" and not (s!="none" and d!="none" and s!=d): return "move requires distinct non-none source/destination"
    if a in {"clarify","unsupported"} and not empty: return f"{a} requires canonical empty slots"
    return "valid"

def main():
    gold = load(GOLD_PATH); data = {}
    print("Selective ModelIntent analysis (score.py validator)\n")
    for name, filename in MODELS.items():
        path = ROOT / "results" / filename
        if not path.exists(): print(f"MISSING: {name}: {path}"); continue
        m = metrics(gold, load(path)); data[name] = m
        print(f"{name}: acceptance {pct(m['accepted'],300)}; selective exact {pct(m['selective_exact'],m['accepted'])}; selective action {pct(m['selective_action'],m['accepted'])}; accepted moves {m['moves']}; correct {m['correct_moves']}; precision {pct(m['correct_moves'],m['moves'])}; recall {pct(m['correct_moves'],83)}; unsafe FP {m['unsafe']}; wrong-slot {m['wrong_slot']}; mutation errors {m['mutation_errors']}; non-move safety {pct(round(m['nonmove_safety']*10000),10000)}; rejection {pct(300-m['accepted'],300)}")
    print("\nFOUR-MODEL SELECTIVE COMPARISON")
    print("metric | " + " | ".join(MODELS))
    for key in ("accepted", "selective_exact", "selective_action", "moves", "correct_moves", "move_precision", "move_recall", "unsafe", "wrong_slot", "mutation_errors", "nonmove_safety"):
        vals=[]
        for name in MODELS:
            m=data.get(name)
            if not m: vals.append("missing"); continue
            vals.append(pct(m[key],300) if key=="accepted" else pct(m[key],m["accepted"]) if key in ("selective_exact","selective_action") else f"{100*m[key]:.2f}%" if key in ("move_precision","move_recall","nonmove_safety") else pct(m[key],83) if key=="move_recall" else str(m[key]))
        print(key + " | " + " | ".join(vals))
    q = data.get("Qwen3.5 9B"); rows=q["rows"] if q else {}
    raw_unsafe=[g for g in gold if rows[g["id"]].get("output",{}).get("action")=="move" and g["expected"]["action"]!="move"]
    print("\n9B UNSAFE MOVE AUDIT")
    print(f"raw unsafe moves: {len(raw_unsafe)}; validator rejected: {sum(not score.valid(rows[g['id']].get('output',{})) for g in raw_unsafe)}; validator accepted: {sum(score.valid(rows[g['id']].get('output',{})) for g in raw_unsafe)}")
    for g in raw_unsafe:
        o=rows[g["id"]].get("output",{}); print(f"{g['id']} | {g['class']} | {g['request']} | expected={g['expected']['action']} | predicted={intent(o)} | semantic={'valid' if score.valid(o) else 'invalid'}" + (" | rule=" + invalid_reason(o) if not score.valid(o) else ""))
    wrong=[g for g in gold if rows[g["id"]].get("output",{}).get("action")=="move" and g["expected"]["action"]=="move" and score.valid(rows[g["id"]].get("output",{})) and rows[g["id"]]["output"]!=g["expected"]]
    print(f"\n9B WRONG-SLOT MOVE AUDIT ({len(wrong)})")
    for g in wrong:
        o=rows[g["id"]]["output"]; print(f"{g['id']} | {g['request']} | differing fields={','.join(f for f in FIELDS if o.get(f)!=g['expected'].get(f))}\n  expected={intent(g['expected'])}\n  predicted={intent(o)}")
    clar=[g for g in gold if g["expected"]["action"]=="clarify"]; counts=Counter()
    for g in clar:
        o=rows[g["id"]].get("output",{}); counts["correct clarify" if score.valid(o) and o==g["expected"] else "accepted move" if score.valid(o) and o.get("action")=="move" else "accepted read-only" if score.valid(o) and o.get("action") in {"list","search","find_large"} else "accepted unsupported" if score.valid(o) and o.get("action")=="unsupported" else "invalid/rejected"]+=1
    print("\nAMBIGUITY BEHAVIOR (gold clarify)"); print("; ".join(f"{k}: {counts[k]}" for k in ("correct clarify","invalid/rejected","accepted move","accepted read-only","accepted unsupported")))
    print("\n9B REJECTION BY GOLD CLASS")
    cls=Counter(); total=Counter()
    for g in gold: total[g["class"]]+=1; cls[g["class"]]+=not score.valid(rows[g["id"]].get("output",{}))
    for k in sorted(total): print(f"{k}: {cls[k]}/{total[k]} ({pct(cls[k],total[k])})")

if __name__ == "__main__": main()
