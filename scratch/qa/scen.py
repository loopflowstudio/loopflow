from drv import *
import norm, re
tag=sys.argv[1]; c=Conn(tag); d=c.repo(); root=os.path.realpath(c.root)
log=[]
def say(name, val):
    s=json.dumps(val,sort_keys=True) if not isinstance(val,str) else val
    s=re.sub(r"/lf-qa-978/(new|old)/","/lf-qa-978/X/",s); s=re.sub(r"home_[0-9a-f]+","home_X",s)
    log.append((name,s)); print(f"[{name}] {s[:900]}",flush=True)
k=[0]
def wave():
    k[0]+=1; r=c.req(f"w{k[0]}",{"kind":"wave.plan","directory":d,"wave":"alpha"})
    if r is None:
        errs=[m for m in c.other if m.get("id")==f"w{k[0]}"]; return {"error":errs[-1]["message"] if errs else "timeout"}
    return r["result"]
def brief(res):
    if "error" in res: return res
    v=norm.view(res); s=v["setup"]
    from collections import Counter
    return {"agent":s["agent"]["value"],"agent_from":s["agent"]["source"]+"/"+str(s["agent"]["scope"]),"graph":s["graph"]["value"],
            "skill_agents":{n:a["value"] for n,a in s["skill_agents"].items()},"providers":{str(a):b for a,b in Counter(v["providers"]).items()},"ntasks":v["ntasks"]}
def write(path, text):
    open(path,"w").write(text)
def settle(): time.sleep(2.2)   # the refresh interval is 1 s

# known starting state
import shutil
if os.path.exists(f"{d}/.lf/config.yaml.gone"): os.rename(f"{d}/.lf/config.yaml.gone", f"{d}/.lf/config.yaml")
subprocess.run(["git","checkout","-q","--",".lf"],cwd=d); shutil.rmtree(f"{d}/.lf/skills/sweep",ignore_errors=True); shutil.rmtree(f"{d}/.lf/skills/scrub",ignore_errors=True)
subprocess.run(["git","checkout","-q","--",".lf"],cwd=f"{root}/wt1")
lf(tag,"config","set","agent","claude:opus"); time.sleep(2.2)
wcfg=f"{d}/.lf/wave/alpha/config.yaml"; rcfg=f"{d}/.lf/config.yaml"; pcfg=f"{d}/lf.config.local.yaml"
orig_w=open(wcfg).read(); orig_r=open(rcfg).read(); orig_p=open(pcfg).read()
say("personal file", orig_p)
# subscriptions that stay open the whole run
c.sub("plan",{"kind":"plan.list","directory":d,"wave":"alpha","task":None,"all":False})
time.sleep(3)
say("plan subscription readings at start", c.n("plan"))
say("1 first read", brief(wave()))
say("1 bare CLI same moment", brief(json.loads(lf(tag,"wave","status","alpha","--json").stdout)))
# 2 remove the personal override: the Wave's agent should win
r=lf(tag,"config","unset","agent"); say("2 config unset agent", r.stdout.strip()+r.stderr.strip())
settle(); say("2 after personal unset", brief(wave()))
# 3 edit the Wave layer
n0=c.n("plan")
write(wcfg, orig_w.replace("claude:sonnet","codex:gpt-5"))
settle(); say("3 after wave agent edit", brief(wave())); say("3 plan subscription woke", c.n("plan")>n0)
# 4 edit skill agent in repository layer
write(rcfg, orig_r.replace("claude:haiku","claude:opus"))
settle(); say("4 after repo skill agent edit", brief(wave()))
# 5 add a new skill to a layer + new skill file
os.makedirs(f"{d}/.lf/skills/sweep",exist_ok=True)
write(f"{d}/.lf/skills/sweep/SKILL.md","---\ndescription: Sweep.\n---\nSweep.\n")
write(rcfg, orig_r.replace("claude:haiku","claude:opus")+"  sweep:\n    agent: claude:haiku\n")
settle(); say("5 after adding skill sweep", brief(wave()))
r=c.req("sk1",{"kind":"skill.list","directory":d,"path":[]}); 
names=lambda r: sorted(re.findall(r'"name": "([\w-]+)"', json.dumps(r["result"]))) if r else None
before=names(r); say("5 skill.list has sweep/tidy", [n for n in before if n in ("sweep","tidy","scrub")])
# 6 new skill file only
os.makedirs(f"{d}/.lf/skills/scrub",exist_ok=True)
write(f"{d}/.lf/skills/scrub/SKILL.md","---\ndescription: Scrub.\n---\nScrub.\n")
settle(); r=c.req("sk2",{"kind":"skill.list","directory":d,"path":[]}); say("6 skill.list after adding scrub", [n for n in names(r) if n in ("sweep","tidy","scrub")])
shutil.rmtree(f"{d}/.lf/skills/scrub")
settle(); r=c.req("sk3",{"kind":"skill.list","directory":d,"path":[]}); say("6 skill.list after removing scrub", [n for n in names(r) if n in ("sweep","tidy","scrub")])
# 7 invalid YAML
write(wcfg, orig_w+"agent: [unclosed\n")
settle(); say("7 invalid wave yaml (served)", brief(wave()))
r=lf(tag,"wave","status","alpha","--json"); say("7 invalid wave yaml (bare)", (r.stderr.strip() or r.stdout[:200]))
# 8 invalid value (valid YAML, bad setting)
write(wcfg, orig_w.replace("claude:sonnet","nonesuch:model"))
settle(); say("8 unknown harness (served)", brief(wave()))
r=lf(tag,"wave","status","alpha","--json"); say("8 unknown harness (bare)", r.stderr.strip() or json.dumps(brief(json.loads(r.stdout))))
# 9 repair
write(wcfg, orig_w)
settle(); say("9 after repair", brief(wave()))
# 10 remove the Wave file's agent line + repo file removed
write(wcfg, "\n".join(l for l in orig_w.splitlines() if not l.startswith("agent:"))+"\n")
settle(); say("10 wave agent line removed", brief(wave()))
os.rename(rcfg, rcfg+".gone")
settle(); say("10 repo config removed", brief(wave()))
os.rename(rcfg+".gone", rcfg); write(rcfg, orig_r); write(wcfg, orig_w)
settle(); say("10 restored", brief(wave()))
# 11 personal file added back through the CLI while server runs; --setting does not leak
r=lf(tag,"--setting","agent=codex:gpt-5","config","settings","--json")
try: say("11 bare --setting agent", json.loads(r.stdout)[0]["settings"]["agent"]["value"])
except Exception as e: say("11 bare --setting agent", r.stdout[:300]+r.stderr[:300])
say("11 served after someone's --setting", brief(wave()))
r=c.req("cs",{"kind":"config.settings","directory":d,"wave":"alpha","skill":None,"graph":None,"step":None})
say("11 served config.settings agent", r["result"][0]["settings"]["agent"]["value"] if r else [m for m in c.other if m.get("id")=="cs"])
r=lf(tag,"config","set","agent","claude:opus"); settle(); say("11 personal set again", brief(wave()))
# 12 a worktree's own config differs from main
wt=f"{root}/wt1/.lf/config.yaml"; o=open(wt).read()
write(wt, o.replace("default_agent: codex","default_agent: codex\nagent: codex:gpt-5"))
lf(tag,"config","unset","agent"); settle(); say("12 one worktree overrides agent", brief(wave()))
write(wt,o); lf(tag,"config","set","agent","claude:opus"); settle(); say("12 restored", brief(wave()))
say("plan subscription readings at end", c.n("plan")>0)
say("errors on connection", [m for m in c.other if m.get("type")=="error"][:5])
json.dump(log,open(f"{ROOT}/scen-{tag}.json","w"),indent=1)
