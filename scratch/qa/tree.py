import re,sys
def parse(path):
    txt=open(path).read().split("Call graph:")[1].split("Total number in stack")[0]
    out=[]
    for l in txt.splitlines():
        m=re.match(r"^([ +!:|]*)(\d+) (.+?)(?:  \(in |$)",l)
        if m: out.append((len(m.group(1)),int(m.group(2)),m.group(3)))
    return out
def short(n):
    n=re.sub(r"^_RNv?[A-Za-z0-9]*?Cs[0-9A-Za-z]+_\d+","",n)
    parts=re.findall(r"\d+([a-z_A-Z][a-zA-Z_0-9]*)",n)
    return "::".join(parts[-3:]) if parts else n[:60]
def top(path,pat,exclude=None):
    rows=parse(path); stack=[]; total=0; callers={}
    for d,n,name in rows:
        while stack and stack[-1][0]>=d: stack.pop()
        inside=any(re.search(pat,s[2]) for s in stack)
        stack.append((d,n,name))
        if re.search(pat,name) and not inside:
            total+=n
            anc=[short(s[2]) for s in stack[:-1] if "loopflow" in s[2] and "closure" not in s[2].lower()]
            key=" > ".join(anc[-5:]); callers[key]=callers.get(key,0)+n
    return total,callers
if __name__=="__main__":
    for tag in ("old","new"):
        p=f"/tmp/lf-qa-978/sample-{tag}.txt"
        print("==",tag)
        for pat in ("settings12resolve_with|settings14resolve_layers","settings14validate_value","SkillCatalog","settings7outside","read_wave_setup|wave_detail","sqlite3_step"):
            t,c=top(p,pat); print(f"  {pat}: {t} samples")
            if "-v" in sys.argv and tag=="new" and "resolve" in pat:
                for k,v in sorted(c.items(),key=lambda x:-x[1])[:8]: print("     ",v,k)
