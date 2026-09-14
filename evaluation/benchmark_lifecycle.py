#!/usr/bin/env python3
"""Sequential benchmark lifecycle wrapper with bounded cleanup."""
import atexit, os, signal, subprocess, sys
MODEL='llama3.2:3b'
def cleanup(*_):
    subprocess.run(['ollama','stop',MODEL],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=30)
    out=subprocess.run(['pgrep','-f','/usr/local/lib/ollama/llama-server'],capture_output=True,text=True).stdout.split()
    for pid in out:
        if pid != str(os.getpid()):
            try: os.kill(int(pid),signal.SIGTERM)
            except (ProcessLookupError,ValueError): pass
atexit.register(cleanup)
for s in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP): signal.signal(s,lambda sig,frame: (cleanup(),sys.exit(128+sig)))
if len(sys.argv)<2: raise SystemExit('usage: benchmark_lifecycle.py COMMAND...')
if subprocess.run(['pgrep','-f','python3 .*evaluation/.*run_.*ollama'],capture_output=True).returncode==0: raise SystemExit('benchmark already running')
subprocess.run(['ollama','ps'],check=False)
raise SystemExit(subprocess.run(sys.argv[1:]).returncode)
