import re,sys
txt=open(sys.argv[1]).read().split("\n")
start=next(i for i,l in enumerate(txt) if "Thread_" in l and "main" in l)
end=next(i for i,l in enumerate(txt) if i>start and re.match(r"\s+\d+ Thread_",l))
lo=int(sys.argv[2]) if len(sys.argv)>2 else 25
maxd=int(sys.argv[3]) if len(sys.argv)>3 else 90
def short(name):
    if not name.startswith("_R"): return "["+name+"]"
    ws=re.findall(r"\d+([A-Za-z_][A-Za-z0-9_]{2,})",name)
    ws=[w for w in ws if not w.startswith("Cs") and w not in("core","future","closure","function","FnOnce","call_once","tokio","runtime","poll","commands","alloc","vec","iter","adapters","result","traits","loopflow")]
    ws=[re.sub(r"^[A-Za-z0-9]+_\d+loopflow","",w) for w in ws]
    return " ".join(w for w in ws[:6] if w)
last=None
for l in txt[start:end]:
    m=re.match(r"^(\s+[+!:| ]*)(\d+) (\S+)",l)
    if not m: continue
    d=len(m.group(1)); n=int(m.group(2)); name=m.group(3)
    if n>=lo and d<=maxd and ("loopflow" in name or name in ("posix_spawn","waitpid","wait4","__wait4","read","sqlite3_step","stat","lstat","open","fork","__posix_spawn")):
        k=short(name)
        if k!=last: print("%3d %5d %s"%(d,n,k[:120])); last=k
