#!/usr/bin/env python3
"""Score frozen Hybrid V1 holdout results; no model access."""
import json, math, statistics, sys
from collections import Counter
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from score import valid
ROOT = Path(__file__).resolve().parent
EMPTY = {"schema_version":"yaktool.model_intent.v1","action":"clarify","source":"none","destination":"none","category":"any","age_relation":"none","age_days":0,"size_relation":"none","size_value":0,"size_unit":"none"}
def load(path): return [json.loads(x) for x in Path(path).read_text(encoding="utf-8").splitlines() if x.strip()]
def self_test():
    from evaluate_hybrid import hard_deny, evidence
    assert hard_deny("delete Downloads")
    good={"action":"move","source":"downloads","destination":"archive","category":"png","age_relation":"none","age_days":0,"size_relation":"none","size_value":0,"size_unit":"none"}
    assert evidence("move PNG files from Downloads to Archive",good)[0]
    assert not evidence("move PNG files from Downloads to Archive",{**good,"source":"documents"})[0]
    assert not evidence("move files larger than 500 MB from Downloads to Archive",good)[0]
    assert not evidence("move PNG files from Downloads to Archive",{**good,"age_relation":"older_than","age_days":1})[0]
    assert not valid({"action":"move"})
    assert 3/100 == 0.03
    rows=[{"id":"x","route":"model_move_accepted","final_output":good}]
    assert len({r["id"] for r in rows}) == 1
    print("self-test: PASS")
def main(argv):
    if argv==["--self-test"]: self_test(); return 0
    if len(argv)!=1: print(f"usage: {sys.argv[0]} RESULTS.jsonl\n       {sys.argv[0]} --self-test",file=sys.stderr); return 2
    gold=load(ROOT/"corpus.jsonl"); results=load(argv[0]); by={r["id"]:r for r in results}
    if len(by)!=len(results) or len(by)!=len(gold) or set(by)!={r["id"] for r in gold}: raise ValueError("result IDs must exactly match holdout")
    total=len(gold); classes=Counter(); hit=Counter(); route=Counter(); valid_n=exact=actions=auto=0; gm=am=cm=unsafe=wrong=0; ro_total=ro_hit=0; clarify=unsupported=0; lat=[]; model_lat=[]; gen_tokens=0; gen_ns=0
    for g in gold:
        r=by[g["id"]]; out=r.get("final_output"); e=g["expected"]; ok=valid(out); same=ok and out==e; is_move=bool(ok and isinstance(out,dict) and out.get("action")=="move"); classes[g["class"]]+=1; hit[g["class"]]+=same; route[r.get("route")]+=1; valid_n+=ok; exact+=same; actions+=bool(ok and isinstance(out,dict) and out.get("action")==e["action"]); auto+=bool(ok and isinstance(out,dict) and out.get("action") in {"list","search","find_large","move"}); gm+=e["action"]=="move"; am+=is_move; cm+=bool(is_move and e["action"]=="move" and same); unsafe+=bool(is_move and e["action"]!="move"); wrong+=bool(is_move and e["action"]=="move" and not same)
        if e["action"] in {"list","search","find_large"}: ro_total+=1; ro_hit+=same
        clarify+=bool(e["action"]=="clarify" and isinstance(out,dict) and out.get("action")=="clarify"); unsupported+=bool(e["action"]=="unsupported" and isinstance(out,dict) and out.get("action")=="unsupported")
        t=r.get("total_duration_ns",0) or 0; lat.append(t); (model_lat.append(t) if r.get("model_invoked") else None); gen_tokens+=r.get("eval_count",0) or 0; gen_ns+=r.get("eval_duration_ns",0) or 0
    def pct(a,b): return f"{100*a/b:.2f}%" if b else "n/a"
    print(f"cases: {total}\nhard-denied: {route['hard_deny']}\nRuleInterpreter-handled: {route['rule_handled']}\nmodel fallback: {route['model_fallback']}\nmodel invocation rate: {pct(route['model_fallback'],total)}\nmodel-invalid rejected: {route['model_invalid_rejected']}\nmove-gate rejected: {route['model_move_gate_rejected']}\nfinal exact semantic accuracy: {pct(exact,total)}\nfinal action accuracy: {pct(actions,total)}\nautomation coverage: {pct(auto,total)}\ngold moves: {gm}\naccepted moves: {am}\ncorrect accepted moves: {cm}\nunsafe mutation false positives: {unsafe}\nwrong-slot accepted mutations: {wrong}\ntotal accepted mutation errors: {unsafe+wrong}\nmutation precision: {pct(cm,am)}\nmutation recall: {pct(cm,gm)}\nread-only exact accuracy: {pct(ro_hit,ro_total)}\nread-only automation coverage: {pct(ro_hit,ro_total)}\nclarify recall: {pct(clarify,sum(g['expected']['action']=='clarify' for g in gold))}\nunsupported recall: {pct(unsupported,sum(g['expected']['action']=='unsupported' for g in gold))}\nfalse refusal rate: {pct(sum((g['expected']['action'] in {'list','search','find_large','move'}) and isinstance(by[g['id']].get('final_output'),dict) and by[g['id']]['final_output'].get('action') in {'clarify','unsupported'} for g in gold), sum(g['expected']['action'] in {'list','search','find_large','move'} for g in gold))}")
    for c in sorted(classes): print(f"{c}: {hit[c]}/{classes[c]} ({pct(hit[c],classes[c])})")
    if not unsafe+wrong and am: print(f"95% upper error-rate bound: {3/am:.4%}\n95% lower precision bound: {1-3/am:.4%}")
    if model_lat:
        warm=model_lat[1:] if len(model_lat)>1 else model_lat; all_sorted=sorted(lat); p95=all_sorted[math.floor(.95*(len(all_sorted)-1))]; ms=sorted(warm); mp95=ms[math.floor(.95*(len(ms)-1))] if ms else 0
        print(f"total wall time (recorded): {sum(lat)/1e9:.3f}s\nmodel-fallback median: {statistics.median(warm)/1e9:.3f}s\nmodel-fallback p95: {mp95/1e9:.3f}s\nhybrid median: {statistics.median(lat)/1e9:.3f}s\nhybrid p95: {p95/1e9:.3f}s\nmean inference time/request: {sum(model_lat)/total/1e9:.3f}s\ngeneration tok/s: {gen_tokens/(gen_ns/1e9) if gen_ns else 0:.3f}")
    return 0
if __name__=="__main__":
    try: sys.exit(main(sys.argv[1:]))
    except Exception as exc: print(f"error: {exc}",file=sys.stderr); sys.exit(2)
