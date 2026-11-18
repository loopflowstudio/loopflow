import json,sys,re
def providers(d):
    out=[]
    def walk(o):
        if isinstance(o,dict):
            if "provider" in o and "title" not in o: out.append(o["provider"])
            for v in o.values(): walk(v)
        elif isinstance(o,list):
            for v in o: walk(v)
    walk(d); return out
def view(d):
    return {"setup": json.loads(re.sub(r"/lf-qa-978/(new|old)/","/lf-qa-978/X/",re.sub(r"home_[0-9a-f]+","home_X",json.dumps(d["setup"])))),
            "providers": providers(d.get("tasks")), "ntasks": len((d.get("tasks") or {}).get("items") or [])}
if __name__=="__main__":
    print(json.dumps(view(json.load(sys.stdin)),sort_keys=True,indent=1))
