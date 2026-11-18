from drv import *
import norm, re, shutil
from collections import Counter
tag=sys.argv[1]; c=Conn(tag); d=c.repo(); root=os.path.realpath(c.root)
log=[]
def say(name, val):
    s=json.dumps(val,sort_keys=True) if not isinstance(val,str) else val
    s=re.sub(r"/lf-qa-978/(new|old)/","/lf-qa-978/X/",s); s=re.sub(r"home_[0-9a-f]+","home_X",s)
    log.append((name,s)); print(f"[{name}] {s[:700]}",flush=True)
k=[0]
def ask(op):
    k[0]+=1; i=f"q{k[0]}"; r=c.req(i,op)
    if r is None:
        errs=[m for m in c.other if m.get("id")==i]; return {"error":errs[-1]["message"] if errs else "timeout"}
    return r["result"]
def wave(): return ask({"kind":"wave.plan","directory":d,"wave":"alpha"})
def brief(res):
    if "error" in res: return res
    v=norm.view(res); s=v["setup"]
    return {"agent":s["agent"]["value"],"skill_agents":{n:a["value"] for n,a in s["skill_agents"].items()},"providers":{str(a):b for a,b in Counter(v["providers"]).items()}}
def bare():
    r=lf(tag,"wave","status","alpha","--json")
    try: return brief(json.loads(r.stdout))
    except Exception: return {"error": sorted(set(r.stderr.strip().splitlines()))[-1] if r.stderr.strip() else r.stdout[:200]}
def write(p,t): open(p,"w").write(t)
def settle(): time.sleep(2.2)
subprocess.run(["git","checkout","-q","--",".lf"],cwd=d)
for i in range(1,7): subprocess.run(["git","checkout","-q","--",".lf"],cwd=f"{root}/wt{i}")
lf(tag,"config","unset","agent"); settle()
wcfg=f"{d}/.lf/wave/alpha/config.yaml"; rcfg=f"{d}/.lf/config.yaml"; ow=open(wcfg).read(); orr=open(rcfg).read()
say("0 start", brief(wave()))
# a worktree's own committed Wave file
w3=f"{root}/wt3/.lf/wave/alpha/config.yaml"; o3=open(w3).read()
write(w3,o3.replace("claude:sonnet","codex")); settle(); say("1 wt3 wave agent -> codex", brief(wave()))
write(w3,o3); settle(); say("1 wt3 restored", brief(wave()))
# unreadable file
os.chmod(wcfg,0); settle(); say("2 unreadable wave file (served)", brief(wave())); say("2 unreadable wave file (bare)", bare())
os.chmod(wcfg,0o644); settle(); say("2 readable again", brief(wave()))
# whole document is not a mapping
write(rcfg,"- just\n- a list\n"); settle(); say("3 repo config is a list (served)", brief(wave())); say("3 repo config is a list (bare)", bare())
say("3 served config.settings", ask({"kind":"config.settings","directory":d,"wave":"alpha","skill":None,"graph":None,"step":None}) if True else None)
write(rcfg,orr); settle(); say("3 repaired", brief(wave()))
# invalid graph name in the Wave layer
write(wcfg,ow+"graph: nonesuch\n"); settle(); r=wave(); say("4 unknown graph (served)", {"graph":r["setup"]["graph"]["value"]} if "error" not in r else r); 
r2=lf(tag,"wave","status","alpha","--json"); 
try: say("4 unknown graph (bare)", {"graph":json.loads(r2.stdout)["setup"]["graph"]["value"]})
except Exception: say("4 unknown graph (bare)", r2.stderr.strip().splitlines()[-1])
write(wcfg,ow); settle(); say("4 repaired", brief(wave()))
# skill front matter edit reaches skill.list
sk=f"{d}/.lf/skills/tidy/SKILL.md"; osk=open(sk).read()
def desc():
    out=[]
    def walk(o):
        if isinstance(o,dict):
            if o.get("name")=="tidy": out.append(o.get("description"))
            for v in o.values(): walk(v)
        elif isinstance(o,list):
            for v in o: walk(v)
    walk(ask({"kind":"skill.list","directory":d,"path":[]})); return out
say("5 tidy before", desc())
write(sk,osk.replace("Tidy the fixture.","Scrub every corner.")); settle(); say("5 tidy after description edit", desc())
write(sk,osk); settle(); say("5 tidy restored", desc())
# task.get subscription on a checked-out Task follows a settings edit
ident=[t["task"]["identifier"] for t in wave()["tasks"]["items"] if t["task"]["name"]=="Checked out task 2"][0]
c.sub("tg",{"kind":"task.get","directory":d,"issue":ident}); time.sleep(4)
prov=lambda f: sorted(set(re.findall(r'"(?:provider|agent|harness)": "([\w:.-]+)"', json.dumps(f))))
n=c.n("tg"); say("6 task.get first", {"readings":n,"agent fields":prov(c.last("tg")) if n else None, "errors":[m for m in c.other if m.get("id")=="tg"]})
w2=f"{root}/wt2/.lf/wave/alpha/config.yaml"; o2=open(w2).read(); write(w2,o2.replace("claude:sonnet","codex"))
woke=c.wait_new("tg",n,8); say("6 task.get after its checkout's Wave file edit", {"woke":woke,"agent fields":prov(c.last("tg")) if c.n("tg") else None})
write(w2,o2); settle()
lf(tag,"config","set","agent","claude:opus")
say("errors", [m for m in c.other if m.get("type")=="error"][:5])
json.dump(log,open(f"{ROOT}/scen2-{tag}.json","w"),indent=1)
