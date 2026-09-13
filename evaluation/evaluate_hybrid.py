#!/usr/bin/env python3
"""One-shot, read-only rule-first hybrid simulation over frozen predictions."""
import json, re, subprocess
from collections import Counter
from pathlib import Path

ROOT=Path(__file__).resolve().parent; GOLD=ROOT/"model-intent-v1.jsonl"; RESULTS=ROOT/"results"
MODELS={"Qwen 0.8B":"qwen35-08b-text-q5-baseline.jsonl","Qwen2.5 3B":"qwen2.5-3b-instruct-baseline.jsonl","Llama 3.2 3B":"llama3.2-3b-baseline.jsonl","Qwen3.5 9B":"qwen3.5-9b-baseline.jsonl"}
BINARY=Path("/tmp/yaktool-hybrid-eval/target/release/hybrid_eval")
LOCS="home|desktop|documents|downloads|pictures|archive"; CAT={"pdf":"pdf","png":"png","jpegs?":"jpeg","jpg":"jpeg","text":"text"}

def load(p): return [json.loads(x) for x in Path(p).read_text(encoding="utf-8").splitlines()]
def hard_deny(s):
    low=s.lower()
    pats=[r"\b(delete|erase|wipe)\b",r"\bremove\s+(?:the\s+)?(?:files?|junk|originals?)\b",r"\b(copy|duplicate|overwrite)\b",r"\b(shell|sudo|chmod|chown)\b",r"\b(run|execute)\s+(?:the\s+)?(?:shell\s+)?command\b",r"\b(?:install|uninstall)\b",r"\b(?:restart|stop|start)\s+(?:the\s+)?(?:service|ssh)\b",r"\bfollow\s+symlinks?\b",r"\b(?:cancel|abort)\s+(?:the\s+)?move\b",r"\b(?:do\s+not|don't|never)\s+move\b",r"\bmove\s+nothing\b",r"\b(?:except|excluding|but\s+not)\b"]
    return next((p for p in pats if re.search(p,low)),None)
def loc(x): return x.lower()
def evidence(req, out):
    s=req.lower(); m=re.search(r"\b(move|put|relocate)\b",s)
    if not m: return False,"no explicit move verb"
    rel=re.search(r"\bfrom\s+(%s)\s+(?:to|into|in)\s+(%s)\b"% (LOCS,LOCS),s)
    if not rel:
        return False,"source/destination relation not explicit"
    if out.get("source")!=rel.group(1) or out.get("destination")!=rel.group(2): return False,"proposal source/destination differs"
    found=[]
    if re.search(r"\bpdf(?:s|\s+files?)?\b",s): found.append("pdf")
    if re.search(r"\bpng(?:\s+files?)?\b",s): found.append("png")
    if re.search(r"\b(?:jpg|jpeg)(?:\s+files?)?\b",s): found.append("jpeg")
    if re.search(r"\btext(?:\s+files?)?\b",s): found.append("text")
    if len(set(found))>1: return False,"contradictory categories"
    cat=found[0] if found else "any"
    if out.get("category")!=cat: return False,"category not evidenced exactly"
    age=None
    am=re.search(r"\b(older|newer)\s+than\s+(\d+)\s+days?\b",s)
    if am: age=("older_than" if am.group(1)=="older" else "newer_than",int(am.group(2)))
    elif re.search(r"\bmodified\s+today\b",s): age=("today",0)
    elif re.search(r"\bmodified\s+this\s+week\b",s): age=("this_week",0)
    if age:
        if out.get("age_relation")!=age[0] or out.get("age_days")!=age[1]: return False,"age constraint mismatch"
    elif out.get("age_relation")!="none" or out.get("age_days")!=0: return False,"unmentioned age constraint"
    sm=re.search(r"\blarger\s+than\s+(\d+)\s+(KB|MB|GB|KiB|MiB|GiB)\b",req,re.I)
    if sm:
        if out.get("size_relation")!="larger_than" or out.get("size_value")!=int(sm.group(1)) or out.get("size_unit")!=sm.group(2): return False,"size constraint mismatch"
    elif out.get("size_relation")!="none" or out.get("size_value")!=0 or out.get("size_unit")!="none": return False,"unmentioned size constraint"
    return True,"evidence complete"
def semantic_from_normalized(i):
    def alias(v): return v.get("value") if isinstance(v,dict) and v.get("kind")=="directory_alias" else "none"
    f=i.get("filters",{}); age=f.get("age"); ar="none"; days=0
    if age:
        if age.get("relation") in ("older_than","newer_than"): ar=age["relation"]; days=age.get("days",0)
        else: ar=age.get("relation")
    ext=f.get("extensions",[]); cat="any" if not ext else "pdf" if ext==[".pdf"] else "png" if ext==[".png"] else "jpeg" if set(ext)=={".jpg",".jpeg"} else "text" if ext==[".txt"] else "any"
    size="none" if f.get("min_size_bytes") is None else "larger_than"
    # Rule intents are converted only for semantic comparison; byte thresholds map to their exact unit when known.
    val=f.get("min_size_bytes") or 0; unit="none"
    for u,n in (("KB",1000),("MB",1000000),("GB",1000000000),("KiB",1024),("MiB",1048576),("GiB",1073741824)):
        if val and val%n==0: unit=u; val//=n; break
    return {"action":i.get("action"),"source":alias(i.get("source")),"destination":alias(i.get("destination")),"category":cat,"age_relation":ar,"age_days":days,"size_relation":size,"size_value":val,"size_unit":unit}
def model_sem(o): return {k:o.get(k) for k in ("action","source","destination","category","age_relation","age_days","size_relation","size_value","size_unit")}
def invoke(rows):
    payload="".join(json.dumps({"request":r["request"],"output":r.get("output",{})})+"\n" for r in rows)
    p=subprocess.run([str(BINARY)],input=payload,text=True,capture_output=True,check=True)
    return [json.loads(x) for x in p.stdout.splitlines()]
def main():
    gold=load(GOLD); allstats={}
    for name,file in MODELS.items():
        pred=load(RESULTS/file); rust=invoke([{"request":g["request"],"output":p.get("output",{})} for g,p in zip(gold,pred)])
        stats=Counter(); finals=[]; routes=[]; model_time=[]; all_lat=[]; gate_reasons=Counter(); rows_by_id={}
        for g,p,r in zip(gold,pred,rust):
            o=p.get("output",{}); low=g["request"].lower(); deny=hard_deny(g["request"])
            if deny: route="hard_deny"; all_lat.append(0); final={"action":"unsupported","source":"none","destination":"none","category":"any","age_relation":"none","age_days":0,"size_relation":"none","size_value":0,"size_unit":"none"}; stats["hard_deny"]+=1
            elif r["rule"] in ("List","Search","FindLarge","Move"):
                route="rule_handled"; all_lat.append(0); final=semantic_from_normalized(r["rule_intent"]); stats["rule_handled"]+=1
            else:
                stats["model_fallback"]+=1; model_time.append(p.get("total_duration_ns",0)); all_lat.append(p.get("total_duration_ns",0));
                if not r["model_valid"]: route="model_invalid_rejected"; final={"action":"clarify","source":"none","destination":"none","category":"any","age_relation":"none","age_days":0,"size_relation":"none","size_value":0,"size_unit":"none"}; stats[route]+=1
                else:
                    final=model_sem(o); a=o.get("action")
                    if a=="move":
                        ok,reason=evidence(g["request"],o)
                        if ok: route="model_move_accepted"; stats[route]+=1
                        else: route="model_move_gate_rejected"; stats[route]+=1; gate_reasons[reason]+=1; final={"action":"clarify","source":"none","destination":"none","category":"any","age_relation":"none","age_days":0,"size_relation":"none","size_value":0,"size_unit":"none"}
                    elif a=="clarify": route="model_clarify"; stats[route]+=1
                    elif a=="unsupported": route="model_unsupported"; stats[route]+=1
                    else: route="model_readonly_accepted"; stats[route]+=1
            exp=model_sem(g["expected"]); same=final==exp
            if same: stats["exact"]+=1
            if final["action"]==exp["action"]: stats["action_correct"]+=1
            if final["action"] in {"list","search","find_large","move"}: stats["automated"]+=1
            if final["action"]=="move":
                stats["accepted_moves"]+=1
                if exp["action"]=="move" and same: stats["correct_moves"]+=1
                elif exp["action"]=="move": stats["wrong_slot"]+=1
                else: stats["unsafe"]+=1
            if exp["action"]!="move" and final["action"]!="move": stats["safe_nonmove"]+=1
            routes.append(route); rows_by_id[g["id"]]=(g,p,r,route,final)
        stats["gate_rejected"]=sum(x=="model_move_gate_rejected" for x in routes); stats["model_invalid"]=sum(x=="model_invalid_rejected" for x in routes); stats["all_lat"]=all_lat
        stats["accepted"]=stats["automated"]; stats["read_only_correct"]=sum(rows_by_id[g["id"]][4]["action"]==model_sem(g["expected"])["action"] and rows_by_id[g["id"]][4]==model_sem(g["expected"]) and model_sem(g["expected"])["action"] in {"list","search","find_large"} for g in gold)
        stats["read_only_total"]=sum(g["expected"]["action"] in {"list","search","find_large"} for g in gold)
        stats["nonmove_total"]=217; stats["gate_reasons"]=gate_reasons; stats["rows"]=rows_by_id; stats["times"]=model_time; allstats[name]=stats
        print(f"{name}: hard_deny={stats['hard_deny']} rule_handled={stats['rule_handled']} fallback={stats['model_fallback']} invalid={stats['model_invalid']} gate_rejected={stats['gate_rejected']} automated={stats['automated']} exact={stats['exact']} action={stats['action_correct']} moves={stats['accepted_moves']} correct_moves={stats['correct_moves']} unsafe={stats['unsafe']} wrong_slot={stats['wrong_slot']} mean_model_time_per_request={sum(model_time)/3e11:.3f}s gate_reasons={dict(gate_reasons)}")
    print("\nSELECTIVE RESULTS")
    print("metric | " + " | ".join(MODELS))
    for label,key in [("Rule-handled","rule_handled"),("Model invocation rate","model_fallback"),("Automation coverage","automated"),("Final exact accuracy","exact"),("Mutation precision","mp"),("Mutation recall","mr"),("Unsafe mutation FP","unsafe"),("Wrong-slot mutation","wrong_slot"),("Total mutation errors","err"),("Read-only accuracy","ro"),("Clarify recall","clarify"),("Unsupported recall","unsupported"),("Hybrid median latency","med"),("Hybrid p95 latency","p95")]:
        vals=[]
        for m in allstats.values():
            if key=="mp": v=100*m["correct_moves"]/m["accepted_moves"] if m["accepted_moves"] else 0; vals.append(f"{v:.2f}%")
            elif key=="mr": vals.append(f"{100*m['correct_moves']/83:.2f}%")
            elif key=="ro": vals.append(f"{100*m['read_only_correct']/m['read_only_total']:.2f}%")
            elif key=="clarify": vals.append(f"{100*sum(m['rows'][g['id']][4]==model_sem(g['expected']) for g in gold if g['expected']['action']=='clarify')/35:.2f}%")
            elif key=="unsupported": vals.append(f"{100*sum(m['rows'][g['id']][4]==model_sem(g['expected']) for g in gold if g['expected']['action']=='unsupported')/85:.2f}%")
            elif key in ("rule_handled","model_fallback"): vals.append(f"{100*m[key]/300:.2f}%")
            elif key=="med" or key=="p95":
                import statistics
                x=sorted(m["all_lat"]); v=statistics.median(x) if key=="med" else x[int(.95*(len(x)-1))]; vals.append(f"{v/1e9:.3f}s")
            elif key=="err": vals.append(str(m['unsafe']+m['wrong_slot']))
            else: vals.append(f"{100*m[key]/300:.2f}%" if key in ("automated","exact","action_correct") else str(m[key]))
        print(label+" | "+" | ".join(vals))
    q=allstats["Qwen3.5 9B"]; print("\n9B GATE REJECTIONS: "+str(dict(q["gate_reasons"])))
    print("9B SIZE CASES (independent MutationEvidenceGate check)")
    for cid in ("case_0146","case_0158","case_0164","case_0170"):
        g,p,r,route,final=q["rows"][cid]; ok,reason=evidence(g["request"],p.get("output",{})) if p.get("output",{}).get("action")=="move" else (False,"model action is not move"); print(cid,"route="+route,"gate="+("pass" if ok else "reject"),reason,"final=",final)
    print("\nROUTING COUNTS (identical expected):", {k:allstats["Qwen3.5 9B"][k] for k in ("hard_deny","rule_handled","model_fallback")})
if __name__=="__main__": main()
