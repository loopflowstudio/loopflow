import json, socket, threading, time, sys, os, subprocess
sys.path.insert(0, "/tmp/lf-qa-978")
ROOT="/tmp/lf-qa-978"
class Conn:
    def __init__(self, tag):
        self.tag=tag; self.root=f"{ROOT}/{tag}"
        st=json.load(open(f"{self.root}/status.json"))
        self.s=socket.socket(socket.AF_UNIX); self.s.connect(st["socket"])
        self.f=self.s.makefile("rwb"); self.lock=threading.Lock()
        self.readings={}; self.responses={}; self.other=[]
        self.send({"type":"hello","revision":8,"min_revision":8,"client":{"kind":"desktop","platform":"macos","name":"qa","version":"0"}})
        self.welcome=json.loads(self.f.readline())
        threading.Thread(target=self._read,daemon=True).start()
    def send(self,m):
        with self.lock: self.f.write(json.dumps(m).encode()+b"\n"); self.f.flush()
    def _read(self):
        for line in self.f:
            m=json.loads(line); t=m.get("type")
            if t=="reading":
                self.readings.setdefault(m["id"],[]).append((time.time(),m["frame"]))
                # no checkpoint needed unless answers requested
            elif t=="response": self.responses[m["id"]]=m
            else: self.other.append(m)
    def sub(self,id,op): self.send({"type":"subscribe","id":id,"operation":op,"select":None,"resume":None,"sender":None})
    def req(self,id,op,wait=20):
        self.send({"type":"request","id":id,"operation":op,"select":None,"sender":None,"request_id":None})
        end=time.time()+wait
        while id not in self.responses and time.time()<end: time.sleep(0.02)
        return self.responses.get(id)
    def n(self,id): return len(self.readings.get(id,[]))
    def last(self,id): return self.readings[id][-1][1]
    def wait_new(self,id,since,timeout=10):
        end=time.time()+timeout
        while self.n(id)<=since and time.time()<end: time.sleep(0.02)
        return self.n(id)>since
    def repo(self): return os.path.realpath(f"{self.root}/repo")
def lf(tag,*args,cwd=None,check=False):
    cmd=f". {ROOT}/env.sh {tag}; cd {cwd or ROOT+'/'+tag+'/repo'}; \"$BIN\" "+" ".join("'"+a.replace("'","'\\''")+"'" for a in args)+" < /dev/null"
    r=subprocess.run(["zsh","-c",cmd],capture_output=True,text=True)
    return r
