#!/usr/bin/env python3
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from hybrid_v2_runtime import parse_move_frame

def main():
 positives=[]
 for v in ('move','relocate','put'):
  for s,d in (('downloads','archive'),('home','documents'),('pictures','desktop'),('archive','home')):
   positives += [f'{v} files from {s} to {d}',f'{v} PDFs from {s} into {d}',f'{v} PNG files older than 7 days from {s} to {d}',f'{v} text files larger than 10 MiB from {s} to {d}',f'{v} JPEG files older than 3 days larger than 2 MB from {s} into {d}']
 negatives=['move files from downloads','move files to archive','copy files from downloads to archive','delete files from downloads to archive','do not move files from downloads to archive','move files from downloads to archive except PDFs','move files from downloads and documents to archive','move files from downloads to archive or desktop','move files from downloads to archive older than 3 days and newer than 1 days','move files from downloads to archive larger than 1 MB larger than 2 MB','move PDF PNG files from downloads to archive','move files from /tmp to archive','move and chmod files from downloads to archive']
 assert len(positives)>=60
 assert all(parse_move_frame(x) and parse_move_frame(x)['action']=='move' for x in positives)
 assert all(parse_move_frame(x) is None for x in negatives)
 print(f'move-frame tests: PASS ({len(positives)+len(negatives)} cases)')
if __name__=='__main__': main()
