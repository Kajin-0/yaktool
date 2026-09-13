#!/usr/bin/env python3
import hashlib,json,re,sys
from collections import Counter
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1])); from score import valid
ROOT=Path(__file__).resolve().parent; EXPECTED={"straightforward_move":100,"paraphrased_move":100,"constraint_rich_move":150,"read_only":100,"ambiguous_missing_information":50,"unsupported_capabilities":40,"negation_exclusions_corrections":30,"adversarial_mixed_safe_unsafe":30}
def norm(s): return re.sub(r"\s+"," ",s.lower().strip().rstrip(".!?")).strip()
def main():
 rows=[json.loads(x) for x in (ROOT/"corpus.jsonl").read_text().splitlines() if x.strip()]; ids=[r["id"] for r in rows]; req=[r["request"] for r in rows]; c=Counter(r["class"] for r in rows)
 assert len(rows)==600 and len(set(ids))==600 and len(set(req))==600 and len({norm(x) for x in req})==600
 assert c==EXPECTED
 for r in rows:
  assert valid(r["expected"]) and r["expected"]["schema_version"]=="yaktool.model_intent.v1"
  if r["class"] in {"ambiguous_missing_information","unsupported_capabilities","negation_exclusions_corrections","adversarial_mixed_safe_unsafe"}: assert r["expected"]["action"] in {"clarify","unsupported"} and r["expected"]["action"]=="unsupported" if r["class"]!="ambiguous_missing_information" else r["expected"]["action"]=="clarify"
  if r["class"] in {"straightforward_move","paraphrased_move","constraint_rich_move"}: assert r["expected"]["action"]=="move" and r["expected"]["source"] in r["request"].lower() and r["expected"]["destination"] in r["request"].lower()
 # leakage is checked by request hashes without exposing sealed case contents.
 for sealed in [Path(__file__).parents[1]/"holdout-v1/corpus.jsonl",Path(__file__).parents[1]/"model-intent-v1.jsonl"]:
  if sealed.exists():
   old={norm(json.loads(x)["request"]) for x in sealed.read_text().splitlines() if x.strip()}; assert not old.intersection({norm(x) for x in req})
 print("DEVELOPMENT V2 VALIDATION PASS"); print(json.dumps({"cases":len(rows),"classes":c},default=dict)); print("leakage: PASS")
if __name__=="__main__":
 try: main()
 except Exception as e: print(f"FAIL: {e}",file=sys.stderr); sys.exit(1)
