"""Manual transport demo against a real provider, in a disposable Loopflow Home."""
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'swift/.build/ghostty-vt/out/aarch64-apple-darwin/lib/libghostty-vt.dylib'
lf = str(ROOT / 'target/debug/lf')
lib = c.CDLL(str(LIB if LIB.exists() else Path('/tmp/loo710-vt.dylib')))
P = c.c_void_p
class Screen(c.Structure):
    _fields_ = [('size', c.c_size_t)] + [(x,c.c_bool) for x in ('cursor','style','hyperlink','protection','kitty_keyboard','charsets')]
class Extra(c.Structure):
    _fields_ = [('size',c.c_size_t)] + [(x,c.c_bool) for x in ('palette','modes','scrolling_region','tabstops','pwd','keyboard')] + [('screen', Screen)]
class Coordinate(c.Structure):
    _fields_ = [('x',c.c_uint16),('y',c.c_uint32)]
class PointValue(c.Union):
    _fields_ = [('coordinate',Coordinate),('padding',c.c_uint64*2)]
class Point(c.Structure):
    _fields_ = [('tag',c.c_int),('value',PointValue)]
class GridRef(c.Structure):
    _fields_ = [('size',c.c_size_t),('node',P),('x',c.c_uint16),('y',c.c_uint16)]
class Selection(c.Structure):
    _fields_ = [('size',c.c_size_t),('start',GridRef),('end',GridRef),('rectangle',c.c_bool)]
class Options(c.Structure):
    _fields_ = [('size', c.c_size_t), ('emit', c.c_int), ('unwrap', c.c_bool), ('trim', c.c_bool), ('extra', Extra), ('selection', P)]
for name,args in {
    'ghostty_terminal_grid_ref':[P,Point,c.POINTER(GridRef)],
    'ghostty_terminal_new':[P,c.POINTER(P),c.c_uint16,c.c_uint16],
    'ghostty_terminal_vt_write':[P,P,c.c_size_t],
    'ghostty_terminal_get':[P,c.c_int,P],
    'ghostty_terminal_set':[P,c.c_int,P],
    'ghostty_formatter_terminal_new':[P,c.POINTER(P),P,Options],
    'ghostty_formatter_format_alloc':[P,P,c.POINTER(P),c.POINTER(c.c_size_t)],
    'ghostty_formatter_free':[P], 'ghostty_terminal_free':[P],
    'ghostty_free':[P,P,c.c_size_t],
}.items():
    getattr(lib,name).argtypes = args

class Terminal:
    def __init__(self):
        self.raw = P()
        assert lib.ghostty_terminal_new(None,c.byref(self.raw),80,24) == 0
        self.replies = bytearray()
        callback = c.CFUNCTYPE(None,P,P,P,c.c_size_t)
        self.callback = callback(lambda t,u,data,n: self.replies.extend(c.string_at(data,n)))
        assert lib.ghostty_terminal_set(self.raw,1,c.cast(self.callback,P)) == 0
    def write(self,data):
        lib.ghostty_terminal_vt_write(self.raw,data,len(data))
    def format(self,emit=0):
        start, end = GridRef(size=c.sizeof(GridRef)), GridRef(size=c.sizeof(GridRef))
        assert lib.ghostty_terminal_grid_ref(self.raw,Point(tag=0,value=PointValue(coordinate=Coordinate(0,0))),c.byref(start))==0
        assert lib.ghostty_terminal_grid_ref(self.raw,Point(tag=0,value=PointValue(coordinate=Coordinate(79,23))),c.byref(end))==0
        selection=Selection(size=c.sizeof(Selection),start=start,end=end)
        options = Options(selection=c.cast(c.byref(selection),P),size=c.sizeof(Options),emit=emit,trim=False,
            extra=Extra(size=c.sizeof(Extra),screen=Screen(size=c.sizeof(Screen))))
        formatter, ptr, n = P(), P(), c.c_size_t()
        assert lib.ghostty_formatter_terminal_new(None,c.byref(formatter),self.raw,options) == 0
        assert lib.ghostty_formatter_format_alloc(formatter,None,c.byref(ptr),c.byref(n)) == 0
        result = c.string_at(ptr,n.value)
        lib.ghostty_free(None,ptr,n)
        lib.ghostty_formatter_free(formatter)
        return result
    def state(self):
        values = {}
        for name,key,typ in [('x',3,c.c_uint16),('y',4,c.c_uint16),('alternate',6,c.c_int),('history',15,c.c_size_t)]:
            val = typ()
            assert lib.ghostty_terminal_get(self.raw,key,c.byref(val)) == 0
            values[name] = val.value
        return values

class Viewer:
    def __init__(self,path,control):
        self.sock=socket.socket(socket.AF_UNIX)
        self.sock.connect(path)
        self.buf=b''
        self.terminal=Terminal()
        self.token=None
        self.control=control
        self.send({'kind':'attach','control':control,'replace':True,'rows':24,'cols':80})
    def send(self,frame):
        self.sock.sendall(json.dumps(frame).encode()+b'\n')
    def input(self,data):
        self.send({'kind':'input','token':self.token,'bytes':list(data)})
    def drain(self,seconds=1):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if not select.select([self.sock],[],[],max(0,end-time.monotonic()))[0]: break
            data=self.sock.recv(1024*1024)
            if not data: break
            self.buf+=data
            while b'\n' in self.buf:
                line,self.buf=self.buf.split(b'\n',1)
                frame=json.loads(line)
                if frame['kind']=='attached': self.token=frame.get('token')
                if frame['kind'] in ('attached','output'):
                    self.terminal.write(bytes(frame['bytes']))
                    if self.control and self.terminal.replies:
                        self.input(self.terminal.replies)
                    self.terminal.replies.clear()

if __name__=='__main__':
    home=Path(tempfile.mkdtemp(prefix='loo710-demo-')).resolve()
    repo=home/'repo'; repo.mkdir()
    subprocess.run(['git','init','-q',str(repo)],check=True)
    env={k:v for k,v in os.environ.items() if not k.startswith(('LF_','LOOPFLOW_','CMUX_'))}
    env.update(LF_HOME=str(home),LF_BIN=lf,TERM='xterm-256color')
    log=(home/'owner.log').open('wb')
    cmd=[lf,'session','serve','--terminal','--',lf,'-i','-a','claude',':','Reply with exactly LOO710-DEMO-READY. Do not use tools.']
    process=subprocess.Popen(cmd,cwd=repo,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=log,start_new_session=True)
    owner=process.stdout.readline().decode().strip()
    assert owner, f'owner failed: {home}'
    path=f'/tmp/lf-{os.geteuid()}-{hashlib.sha256(str(home).encode()).hexdigest()[:16]}/{owner}.sock'
    first=Viewer(path,True)
    first.drain(5)
    if b'Yes, I trust this folder' in first.terminal.format():
        first.input(b'\x1b[B\r')
    first.drain(15)
    print(json.dumps({'home':str(home),'pid':process.pid,'owner':owner,'screen':first.terminal.format().decode(errors='replace'),'state':first.terminal.state()}),flush=True)
    (ROOT/'scratch/demo-live.json').write_text(json.dumps({'home':str(home),'owner':owner,'pid':process.pid,'socket':path}))
    # Keep the first renderer available while a second viewer attaches.
    second=Viewer(path,False)
    second.drain(2);first.drain(.2)
    evidence={'first':first.terminal.format().decode(errors='replace'),'second':second.terminal.format().decode(errors='replace'),'first_state':first.terminal.state(),'second_state':second.terminal.state()}
    evidence['same_visible_text']=evidence['first']==evidence['second']
    evidence['same_html']=first.terminal.format(2)==second.terminal.format(2)
    (ROOT/'scratch/demo-provider-evidence.json').write_text(json.dumps(evidence,indent=2))
    print(json.dumps(evidence),flush=True)
    first.sock.close();second.sock.close()
