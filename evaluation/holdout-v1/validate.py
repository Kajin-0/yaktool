#!/usr/bin/env python3
"""Validate holdout schema, semantics, grounding, uniqueness, and leakage."""
import hashlib, importlib.util, json, re, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent; REPO=ROOT.parents[1]; CORPUS=ROOT/"corpus.jsonl"; DEV=REPO/"evaluation/model-intent-v1.jsonl"; MANIFEST=ROOT/"manifest.json"
spec=importlib.util.spec_from_file_location("score",REPO/"evaluation/score.py"); score=importlib.util.module_from_spec(spec); spec.loader.exec_module(score)
def norm(s): return re.sub(r"\s+"," ",s.strip().rstrip(".!? ")).lower()
def check_ground(r):
    e=r["expected"]; s=r["request"].lower()
    if e["action"]=="move":
        for value in (e["source"],e["destination"]):
            if not re.search(rf"\b{value}\b",s): return False,"location missing"
        if e["category"]!="any" and not re.search(rf"\b{e['category'].replace('jpeg','(?:jpg|jpeg)')}\b",s): return False,"category missing"
        if e["age_relation"]=="older_than" and f"older than {e['age_days']} days" not in s: return False,"age missing"
        if e["age_relation"]=="newer_than" and f"newer than {e['age_days']} days" not in s: return False,"age missing"
        if e["age_relation"]=="today" and "today" not in s: return False,"age missing"
        if e["age_relation"]=="this_week" and "modified this week" not in s: return False,"age missing"
        if e["size_relation"]=="larger_than" and f"larger than {e['size_value']} {e['size_unit'].lower()}" not in s: return False,"size missing"
        if e["size_relation"]=="none" and re.search(r"\blarger than\b",s): return False,"unexpected size"
    elif e["action"] in {"clarify","unsupported"} and not (e["source"]==e["destination"]=="none" and e["category"]=="any" and e["age_relation"]=="none" and e["age_days"]==0 and e["size_relation"]=="none" and e["size_value"]==0 and e["size_unit"]=="none"): return False,"noncanonical empty slots"
    return True,"ok"
def load_dev(): return [json.loads(x) for x in DEV.read_text(encoding="utf-8").splitlines()]
def main():
    rows=[json.loads(x) for x in CORPUS.read_text(encoding="utf-8").splitlines()]; assert len(rows)==1200
    ids=[r["id"] for r in rows]; req=[r["request"] for r in rows]; assert len(set(ids))==1200 and len(set(req))==1200
    assert len({norm(x) for x in req})==1200
    from collections import Counter
    expected={"straightforward_move":250,"paraphrased_move":250,"constraint_rich_move":150,"read_only":200,"ambiguous_missing_information":100,"unsupported_capabilities":100,"negation_exclusions_corrections":75,"adversarial_mixed_safe_unsafe":75}
    assert Counter(r["class"] for r in rows)==expected
    for r in rows:
        assert score.valid(r["expected"]), r["id"]
        ok,why=check_ground(r); assert ok,(r["id"],why)
    dev=load_dev(); assert not ({norm(r["request"]) for r in rows}&{norm(r["request"]) for r in dev})
    m=json.loads(MANIFEST.read_text(encoding="utf-8")); assert m["corpus_sha256"]==hashlib.sha256(CORPUS.read_bytes()).hexdigest()
    print("HOLDOUT VALIDATION PASS"); print("cases: 1200; classes: "+json.dumps(dict(Counter(r["class"] for r in rows)),sort_keys=True)); print("development leakage: PASS (0 normalized duplicates)")
if __name__=="__main__":
    try: main()
    except (AssertionError,KeyError,ValueError) as e: print(f"HOLDOUT VALIDATION FAIL: {e}",file=sys.stderr); sys.exit(1)
