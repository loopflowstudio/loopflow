from drv import *
tag=sys.argv[1]; c=Conn(tag); d=c.repo(); home=f"{c.root}/home/.lf/config.yaml"
k=[0]
def served():
    k[0]+=1; r=c.req(f"g{k[0]}",{"kind":"wave.plan","directory":d,"wave":"alpha"})
    return json.dumps(r["result"]) if r else str([m for m in c.other][-1:])
mid=json.loads(served())["setup"]["tasks_machine"]["value"]
orig=open(home).read() if os.path.exists(home) else None
print("before: served names 'mini':", '"mini"' in served())
open(home,"w").write((orig or "")+f"machines:\n  mini:\n    id: {mid}\n")
time.sleep(2.2); s=served(); b=lf(tag,"wave","status","alpha","--json").stdout
print("alias added: served", s.count('"mini"'), "bare", b.count('"mini"'))
if orig is None: os.remove(home)
else: open(home,"w").write(orig)
time.sleep(2.2); print("alias removed: served", served().count('"mini"'))
