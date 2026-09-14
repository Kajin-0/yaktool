#!/usr/bin/env python3
import json, random
from pathlib import Path
SEED=20260916
LOC=['home','desktop','documents','downloads','pictures','archive']
CATS=[('PDF','pdf'),('PNG','png'),('JPEG','jpeg'),('text','text')]
def empty(action='clarify'):
 return {'schema_version':'yaktool.model_intent.v1','action':action,'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
def main():
 rng=random.Random(SEED); rows=[]
 for cls,count in [('resolvable_move_paraphrase',100),('resolvable_read_only_paraphrase',50),('ambiguous_clarify',30),('unsupported_nontrivial',20)]:
  for _ in range(count):
   s,d=rng.sample(LOC,2)
   if cls=='resolvable_move_paraphrase':
    _,c=rng.choice(CATS); req=f"I'd like the {c} files in {s} moved into {d} (example {len(rows)+1})"; g=empty('move'); g.update(source=s,destination=d,category=c)
   elif cls=='resolvable_read_only_paraphrase':
    _,c=rng.choice(CATS); req=f"Can you show me {c} files sitting in {s}? (example {len(rows)+1})"; g=empty('search'); g.update(source=s,category=c)
   elif cls=='ambiguous_clarify': req=rng.choice(['move my old PDFs','put these somewhere safer','find the file I used yesterday'])+f' (example {len(rows)+1})'; g=empty()
   else: req=rng.choice(['copy Downloads to a USB drive','rename files in Downloads','compress Documents into an archive'])+f' (example {len(rows)+1})'; g=empty('unsupported')
   rows.append({'id':f'fallback_dev_{len(rows)+1:04d}','class':cls,'template_family':cls,'request':req,'expected':g})
 p=Path(__file__).with_name('corpus.jsonl'); p.write_text(''.join(json.dumps(x,separators=(',',':'))+'\n' for x in rows))
 Path(__file__).with_name('manifest.json').write_text(json.dumps({'seed':SEED,'cases':200},indent=2)+'\n')
if __name__=='__main__': main()
