import json
from loo710_demo import ROOT,Viewer
record=json.loads((ROOT/'scratch/demo-live.json').read_text())
first=Viewer(record['socket'],True)
first.drain(1)
first.input(b'\x1b[B\r')
first.drain(15)
print(first.terminal.format().decode(errors='replace'),flush=True)
second=Viewer(record['socket'],False)
second.drain(1);first.drain(.1)
evidence={'first':first.terminal.format().decode(errors='replace'),'second':second.terminal.format().decode(errors='replace'),'first_state':first.terminal.state(),'second_state':second.terminal.state(),'same_visible_text':first.terminal.format()==second.terminal.format(),'same_html':first.terminal.format(2)==second.terminal.format(2)}
(ROOT/'scratch/demo-provider-evidence.json').write_text(json.dumps(evidence,indent=2))
print(json.dumps(evidence),flush=True)
first.sock.close();second.sock.close()
