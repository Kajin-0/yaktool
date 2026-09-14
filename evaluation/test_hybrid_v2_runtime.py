#!/usr/bin/env python3
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from hybrid_v2_runtime import empty, hard_deny, read_only, semantic_load

def main():
 assert hard_deny('delete Downloads')
 assert read_only('show me files in Downloads')['action']=='list'
 assert read_only('find pdf files in Documents')['category']=='pdf'
 assert semantic_load({'source':'downloads','destination':'archive','category':'png','age_relation':'older_than','size_relation':'none'})==4
 assert empty('clarify')['source']=='none'
 print('hybrid runtime self-test: PASS')
if __name__=='__main__': main()
