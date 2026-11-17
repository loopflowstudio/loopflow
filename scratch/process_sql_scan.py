import re, sys, os, json
root = sys.argv[1]
lit = re.compile(r'r(#+)"(.*?)"\1|"((?:[^"\\]|\\.)*)"', re.S)
proc = re.compile(r'(?<![A-Za-z0-9_])processes(?![A-Za-z0-9_])')
fnre = re.compile(r'\bfn\s+([A-Za-z0-9_]+)')
tbl = re.compile(r'(?i)\b(?:from|join|into|update|exists\s*\(\s*select\s+1\s+from)\s+([A-Za-z_][A-Za-z0-9_.]*)')
out=[]
for d,_,fs in os.walk(root):
    if '/tests' in d or 'target' in d: continue
    for f in fs:
        if not f.endswith('.rs') or f.endswith('_tests.rs') or f.startswith('test_') or 'fixture' in f: continue
        p=os.path.join(d,f); s=open(p).read()
        m=re.search(r'#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*mod\s+\w+\s*\{', s)
        if m: s=s[:m.start()]
        for lm in lit.finditer(s):
            body = lm.group(2) if lm.group(2) is not None else lm.group(3)
            if not proc.search(body): continue
            if not re.search(r'(?i)\b(select|insert|update|delete|create|drop|from)\b', body): continue
            line=s.count('\n',0,lm.start())+1
            fns=fnre.findall(s[:lm.start()]); fn=fns[-1] if fns else '?'
            tabs=sorted(set(t.split('.')[-1] for t in tbl.findall(body)))
            kind='write' if re.search(r'(?i)\b(insert\s+(or\s+\w+\s+)?into|update|delete\s+from)\s+processes\b',body) else 'read'
            others=[t for t in tabs if t!='processes']
            out.append(dict(file=os.path.relpath(p,root),line=line,fn=fn,kind=kind,tables=tabs,joined=bool(others)))
json.dump(out,open('/tmp/loo953/scan.json','w'),indent=1)
print(len(out), sum(o['joined'] for o in out), sum(o['kind']=='write' for o in out))
from collections import Counter
print(Counter(o['file'] for o in out).most_common(40))
