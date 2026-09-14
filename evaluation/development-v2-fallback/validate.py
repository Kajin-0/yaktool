#!/usr/bin/env python3
import json,re
from pathlib import Path
rows=[json.loads(x) for x in Path(__file__).with_name('corpus.jsonl').read_text().splitlines()]
norm=lambda s: re.sub(r'\s+',' ',s.lower().strip().rstrip('.'))
assert len(rows)==200 and len({x['id'] for x in rows})==200
assert len({x['request'] for x in rows})==200 and len({norm(x['request']) for x in rows})==200
for x in rows:
 e=x['expected']; assert e['schema_version']=='yaktool.model_intent.v1'
print('fallback development validation PASS',len(rows))
