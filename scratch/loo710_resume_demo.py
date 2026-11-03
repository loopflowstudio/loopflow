import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shutil
import struct
import subprocess
import termios
import time
from loo710_demo import ROOT, Terminal, lf
record=json.loads((ROOT/'scratch/demo-live.json').read_text())
home=Path(record['home']); repo=home/'repo'
session=next((home/'screens').glob('*.ghostsnp')).stem
copy=home/'lf-demo';shutil.copy2(lf,copy)
env={k:v for k,v in os.environ.items() if not k.startswith(('LF_','LOOPFLOW_','CMUX_'))}
env.update(LF_HOME=str(home),LF_BIN=str(copy),TERM='xterm-256color')
master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,0,0))
process=subprocess.Popen([str(copy),'resume',session],cwd=repo,env=env,stdin=slave,stdout=slave,stderr=slave,start_new_session=True)
os.close(slave)
terminal=Terminal();raw=bytearray()
def drain(seconds):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if not select.select([master],[],[],max(0,end-time.monotonic()))[0]: break
        try: data=os.read(master,65536)
        except OSError: break
        if not data: break
        raw.extend(data);terminal.write(data)
        if terminal.replies:
            os.write(master,terminal.replies);terminal.replies.clear()
drain(15)
print(json.dumps({'session':session,'resumed_screen':terminal.format().decode(errors='replace'),'state':terminal.state()}),flush=True)
assert b'LOO710-DEMO-READY' in terminal.format()
saved_before=next((home/'screens').glob('*.ghostsnp')).stat().st_mtime_ns
# This disposable executable is the one the owner watches. Removing it uses
# the real lifetime-policy ending path without signaling unrelated work.
copy.unlink()
drain(12)
snapshot=next((home/'screens').glob('*.ghostsnp'))
evidence={'session':session,'resumed':True,'snapshot_replaced_by_lifetime_policy':snapshot.stat().st_mtime_ns!=saved_before,'snapshot_bytes':snapshot.stat().st_size,'viewer_exit':process.poll(),'exit_screen':terminal.format().decode(errors='replace')}
(ROOT/'scratch/demo-resume-evidence.json').write_text(json.dumps(evidence,indent=2))
print(json.dumps(evidence),flush=True)
if process.poll() is None:
    os.write(master,b'\x1d');drain(2)
os.close(master)
