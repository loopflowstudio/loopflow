import json
import os
from pathlib import Path
import subprocess
from loo710_demo import ROOT, Viewer, lf
record=json.loads((ROOT/'scratch/demo-live.json').read_text())
v=Viewer(record['socket'],True);v.drain(1)
v.input(b'/exit\r');v.drain(5)
v.sock.close()
home=Path(record['home'])
snapshots=list((home/'screens').glob('*.ghostsnp'))
env={k:v for k,v in os.environ.items() if not k.startswith(('LF_','LOOPFLOW_','CMUX_'))}
env.update(LF_HOME=str(home),LF_BIN=lf)
r=subprocess.run([lf,'session','list','--json'],env=env,cwd=home/'repo',capture_output=True,text=True,check=True)
sessions=json.loads(r.stdout)
print(json.dumps({'snapshots':[{'path':str(p),'bytes':p.stat().st_size,'header':p.read_bytes().split(b'\n',1)[0].decode()} for p in snapshots],'sessions':sessions}),flush=True)
(ROOT/'scratch/demo-exit-evidence.json').write_text(json.dumps({'snapshots':[str(p) for p in snapshots],'sessions':sessions,'screen':v.terminal.format().decode(errors='replace')},indent=2))
