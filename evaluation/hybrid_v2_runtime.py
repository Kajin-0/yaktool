"""Canonical evaluation-only Hybrid V2 semantic helpers.

This module contains no filesystem access or model calls.  It centralizes the
deterministic gates used by development evaluators.
"""
import re
from evaluate_hybrid import hard_deny, evidence, semantic_from_normalized

LOCS = r"home|desktop|documents|downloads|pictures|archive"

def empty(action="clarify"):
    return {"schema_version":"yaktool.model_intent.v1","action":action,
            "source":"none","destination":"none","category":"any",
            "age_relation":"none","age_days":0,"size_relation":"none",
            "size_value":0,"size_unit":"none"}

def read_only(request):
    s=request.lower()
    m=(re.search(r"\b(?:in|from|for)\s+("+LOCS+r")\b",s)
       or re.search(r"\b(?:list|show|find|search)\s+("+LOCS+r")\b",s))
    if not m: return None
    prefix=s[:m.end()]
    if re.search(r"\b(find|search)\b",prefix): action="search"
    elif re.search(r"\b(list|show)\b",prefix): action="list"
    else: return None
    category="any"
    cats=[x for x in ("pdf","png","jpeg","text") if re.search(r"\b"+x+r"s?\b",s)]
    if len(cats)==1: category=cats[0]
    age=("none",0); a=re.search(r"\b(older|newer)\s+than\s+(\d+)\s+days?\b",s)
    if a: age=("older_than" if a.group(1)=="older" else "newer_than",int(a.group(2)))
    elif "modified today" in s: age=("today",0)
    elif "modified this week" in s: age=("this_week",0)
    z=("none",0,"none"); sm=re.search(r"\blarger\s+than\s+(\d+)\s+(KB|MB|GB|KiB|MiB|GiB)\b",s,re.I)
    if sm: z=("larger_than",int(sm.group(1)),sm.group(2)); action="find_large"
    if action=="list": category="any"; age=("none",0); z=("none",0,"none")
    return {**empty(action),"source":m.group(1),"category":category,
            "age_relation":age[0],"age_days":age[1],"size_relation":z[0],
            "size_value":z[1],"size_unit":z[2]}

def semantic_load(intent):
    n=2
    if intent.get("category") not in (None,"any"): n+=1
    if intent.get("age_relation") not in (None,"none"): n+=1
    if intent.get("size_relation") not in (None,"none"): n+=1
    return n

def model_intent_from_rule(rule_intent):
    return semantic_from_normalized(rule_intent)
