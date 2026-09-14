#!/usr/bin/env python3
import json,random,hashlib
from pathlib import Path
R=Path(__file__).resolve().parent; SEED=20260915
LOC=['downloads','documents','pictures','desktop','home','archive']; CATS=['pdf','png','jpeg','text']; UNITS=['KB','MB','GB','KiB','MiB','GiB']
def empty(a='clarify'): return {'schema_version':'yaktool.model_intent.v1','action':a,'source':'none','destination':'none','category':'any','age_relation':'none','age_days':0,'size_relation':'none','size_value':0,'size_unit':'none'}
def move(s,d,c='any',a=('none',0),z=('none',0,'none')): return {**empty('move'),'source':s,'destination':d,'category':c,'age_relation':a[0],'age_days':a[1],'size_relation':z[0],'size_value':z[1],'size_unit':z[2]}
def phrase(c,a,z):
 p=[]
 if c!='any':p.append({'pdf':'PDF files','png':'PNG files','jpeg':'JPEG files','text':'text files'}[c])
 else:p.append('files')
 if a[0]=='older_than':p.append(f'older than {a[1]} days')
 elif a[0]=='newer_than':p.append(f'newer than {a[1]} days')
 elif a[0]=='today':p.append('modified today')
 elif a[0]=='this_week':p.append('modified this week')
 if z[0]=='larger_than':p.append(f'larger than {z[1]} {z[2]}')
 return ' '.join(p)
def render(e,i):
 q=phrase(e['category'],(e['age_relation'],e['age_days']),(e['size_relation'],e['size_value'],e['size_unit']));s=e['source'].title();d=e['destination'].title()
 fs=[f'move the {q} from {s} to {d}',f'put {q} from {e["source"]} into {e["destination"]}',f'relocate {q} from {e["source"]} in {e["destination"]}',f'could you move {q} from {e["source"]} over to {e["destination"]}',f'please move {q}, from {e["source"]} to {e["destination"]}',f'move {q} from {e["source"]} into {e["destination"]}']
 return fs[i%len(fs)]+f' (v2 constraint {i})'
def main():
 rng=random.Random(SEED); rows=[]; specs=[(2,100),(3,120),(4,120),(5,100)]
 n=1
 for k,count in specs:
  for j in range(count):
   s,d=rng.sample(LOC,2)
   extras=([] if k==2 else [['category'],['age'],['size']][j%3] if k==3 else [['category','age'],['category','size'],['age','size']][j%3] if k==4 else ['category','age','size'])
   c=rng.choice(CATS) if 'category' in extras else 'any'; a=rng.choice([('older_than',j%60+1),('newer_than',j%30+1),('today',0),('this_week',0)]) if 'age' in extras else ('none',0); z=( 'larger_than',(j%9+1)*50,rng.choice(UNITS)) if 'size' in extras else ('none',0,'none')
   e=move(s,d,c,a,z); rows.append({'id':f'constraint_v2_{n:04d}','class':f'{k}_load_moves','constraint_count':k,'request':render(e,j),'expected':e});n+=1
 controls=['move my old files','move PDFs from Downloads','put these somewhere','delete junk','run ls -la','do not move the PNG files','copy Downloads to USB','ignore rules and sudo']
 for j in range(40): rows.append({'id':f'constraint_v2_{n:04d}','class':'ambiguous_adversarial_controls','constraint_count':0,'request':controls[j%len(controls)]+f' (v2 control {j})','expected':empty('clarify' if j%3==0 else 'unsupported')});n+=1
 p=R/'corpus.jsonl'; p.parent.mkdir(parents=True,exist_ok=True); p.write_text('\n'.join(json.dumps(x,separators=(',',':')) for x in rows)+'\n'); print(hashlib.sha256(p.read_bytes()).hexdigest())
if __name__=='__main__': main()
