#!/usr/bin/env python3
"""Rust-helper contract tests; never contacts Ollama."""
import json, subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
helper=ROOT/'evaluation/hybrid-v2/rust_helper/target/debug/yaktool-v2-helper'
def run(v):
    p=subprocess.run([str(helper)],input=json.dumps(v)+'\n',text=True,capture_output=True,check=True)
    return json.loads(p.stdout)
base={'schema_version':'yaktool.model_intent.v1','action':'move','source':'downloads','destination':'archive','category':'pdf','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
ok=run({'request':'move PDFs from downloads to archive','model_intent':base})
assert ok['model_intent_deserialized'] and ok['model_intent_valid'] and ok['normalization_valid'] and ok['gate_accepted']
bad=dict(base); bad['category']='png'
rejected=run({'request':'move PDFs from downloads to archive','model_intent':bad})
assert rejected['model_intent_valid'] and rejected['gate_applied'] and not rejected['gate_accepted']
invalid=dict(base); invalid['schema_version']='wrong'
assert not run({'request':'move PDFs from downloads to archive','model_intent':invalid})['model_intent_valid']
clarify=dict(base); clarify.update(action='clarify',source='none',destination='none',category='any')
c=run({'request':'move my files','model_intent':clarify})
assert c['model_intent_valid'] and c['final_projection']['action']=='clarify'
assert run({'request':'move PDFs from downloads over to archive'})['route'] in ('rule_interpreter','move_frame')
print('Rust helper self-tests PASS')
