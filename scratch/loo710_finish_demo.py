import ctypes as c
import hashlib
import json
from pathlib import Path
import sqlite3
from loo710_demo import ROOT, Viewer, Terminal, lib, P
record=json.loads((ROOT/'scratch/demo-live.json').read_text());home=Path(record['home'])
con=sqlite3.connect(home/'loopflow.db')
session,owner=con.execute('select id,transport_lf_process_id from agent_sessions').fetchone()
path=f'/tmp/lf-{__import__("os").geteuid()}-{hashlib.sha256(str(home).encode()).hexdigest()[:16]}/{owner}.sock'
v=Viewer(path,True);v.drain(1);v.input(b'/exit\r');v.drain(5)
v.sock.close()
lib.ghostty_snapshot_decoder_new_buf.argtypes=[P,c.POINTER(P),P,c.c_size_t]
lib.ghostty_snapshot_decoder_decode.argtypes=[P,c.POINTER(P)]
lib.ghostty_snapshot_decoder_free.argtypes=[P]
snapshot=home/'screens'/f'{session}.ghostsnp'
header,stream=snapshot.read_bytes().split(b'\n',1)
decoder=P();raw=P()
assert lib.ghostty_snapshot_decoder_new_buf(None,c.byref(decoder),stream,len(stream))==0
assert lib.ghostty_snapshot_decoder_decode(decoder,c.byref(raw))==0
lib.ghostty_snapshot_decoder_free(decoder)
restored=Terminal();lib.ghostty_terminal_free(restored.raw);restored.raw=raw
result={'snapshot_decodes':True,'saved_screen_matches_last_viewer':restored.format()==v.terminal.format(),'saved_screen':restored.format().decode(errors='replace'),'last_viewer':v.terminal.format().decode(errors='replace'),'saved_state':restored.state(),'viewer_state':v.terminal.state()}
(ROOT/'scratch/demo-snapshot-evidence.json').write_text(json.dumps(result,indent=2))
print(json.dumps(result),flush=True)
