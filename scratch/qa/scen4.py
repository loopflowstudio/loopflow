# A Task's checkout leaves and comes back in a repository whose only settings layer is the personal file.
from drv import *
import re
tag=sys.argv[1]; c=Conn(tag); root=os.path.realpath(c.root); d=f"{root}/repo2"; wt=f"{root}/r2wt"
ident=json.load(open(f"{root}/r2task.json"))["task"]["identifier"]
log=[]; k=[0]
def provider(o):
    return {"execution": (o.get("execution") or {}).get("provider"), "row": ((o.get("row") or {}).get("runtime") or {}).get("provider")}
def served():
    k[0]+=1; i=f"t{k[0]}"; r=c.req(i,{"kind":"task.get","directory":d,"issue":ident})
    if r is None: return {"error":[m.get("message") for m in c.other if m.get("id")==i][-1:] or "timeout"}
    return provider(r["result"])
def bare():
    r=lf(tag,"task","show",ident,"--json",cwd=d)
    try: return provider(json.loads(r.stdout))
    except Exception: return {"error": (r.stderr.strip().splitlines() or [r.stdout[:200]])[-1]}
def say(name):
    time.sleep(2.2); n=c.n("tg")
    row={"served":served(),"bare":bare(),"subscription":provider(c.last("tg")["body"]) if n else None}
    s=re.sub(r"/lf-qa-978/(new|old|prev)/","/lf-qa-978/X/",json.dumps(row,sort_keys=True))
    log.append((name,s)); print(f"[{name}] {s}",flush=True)
git=lambda *a: subprocess.run(["git","-C",d,*a],capture_output=True,text=True)
pcfg=f"{d}/lf.config.local.yaml"; orig=open(pcfg).read()
c.sub("tg",{"kind":"task.get","directory":d,"issue":ident}); time.sleep(3)
say("1 checkout present, personal agent claude:opus")
r=git("worktree","remove","--force",wt); assert r.returncode==0, r.stderr
say("2 checkout removed")
r=git("worktree","add",wt,"r2"); assert r.returncode==0, r.stderr
say("3 checkout back")
open(pcfg,"w").write("agent: codex\n")
say("4 personal agent -> codex")
open(pcfg,"w").write(orig)
say("5 personal agent restored")
log.append(("errors",json.dumps([m for m in c.other if m.get("type")=="error"][:5])))
print(log[-1])
json.dump(log,open(f"{ROOT}/scen4-{tag}.json","w"),indent=1)
