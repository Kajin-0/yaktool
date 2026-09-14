#!/usr/bin/env python3
import json,re
from pathlib import Path
rows=[json.loads(x) for x in Path(__file__).with_name('corpus.jsonl').read_text().splitlines()]
assert len(rows)==200 and len({r['id'] for r in rows})==200
marker=re.compile(r'\((?:example|case|test)\s*\d+\)|(?:#|ID)[-_]?\d+',re.I)
assert not any(marker.search(r['request']) for r in rows)
norm=lambda s: re.sub(r'[^a-z0-9 ]','',re.sub(r'\s+',' ',s.lower().strip()))
assert len({r['request'] for r in rows})==200 and len({norm(r['request']) for r in rows})==200
for r in rows:
 e=r['expected']; assert e['schema_version']=='yaktool.model_intent.v1'
print('fallback-v2 validation PASS',len(rows))
