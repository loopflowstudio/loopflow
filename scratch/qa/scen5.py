# A Wave's Task list while one Task's checkout is gone: your own settings still reach that Task.
from drv import *
import norm, re
from collections import Counter
tag=sys.argv[1]; c=Conn(tag); d=c.repo(); root=os.path.realpath(c.root); wt=f"{root}/wt6"
log=[]; k=[0]
def brief(res):
    v=norm.view(res); return {"agent":v["setup"]["agent"]["value"],"providers":{str(a):b for a,b in Counter(v["providers"]).items()}}
def served():
    k[0]+=1; r=c.req(f"w{k[0]}",{"kind":"wave.plan","directory":d,"wave":"alpha"})
    return brief(r["result"]) if r else {"error":[m.get("message") for m in c.other][-1:]}
def bare(*cmd):
    r=lf(tag,*cmd)
    try: return brief(json.loads(r.stdout))
    except Exception: return {"error":(r.stderr.strip().splitlines() or [r.stdout[:200]])[-1]}
def say(name):
    time.sleep(2.2)
    s=json.dumps({"served":served(),"bare wave status":bare("wave","status","alpha","--json")},sort_keys=True)
    s=re.sub(r"/lf-qa-978/(new|old|prev)/","/lf-qa-978/X/",s); log.append((name,s)); print(f"[{name}] {s}",flush=True)
git=lambda *a: subprocess.run(["git","-C",d,*a],capture_output=True,text=True)
lf(tag,"config","set","agent","codex")
say("1 personal agent codex, all checkouts present")
r=git("worktree","remove","--force",wt); assert r.returncode==0, r.stderr
say("2 wt6 removed")
r=git("worktree","add",wt,"wt6"); assert r.returncode==0, r.stderr
say("3 wt6 back")
lf(tag,"config","set","agent","claude:opus")
say("4 personal agent claude:opus")
json.dump(log,open(f"{ROOT}/scen5-{tag}.json","w"),indent=1)
