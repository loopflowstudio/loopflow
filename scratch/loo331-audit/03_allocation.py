import json,os,collections,statistics
runs=json.load(open('runs.json'))
def wave(m):
    for s in m.get('subjects') or []:
        if s['selector'].startswith('wave:'): return s['selector'][5:]
lf=[r for r in runs if r[0]['repo'].startswith('/Users/jack/src/loopflow')]
def q(v):
    v=sorted(v); n=len(v)
    return f"n={n} min={v[0]} p50={v[n//2]} p90={v[int(n*.9)]} max={v[-1]} sum={sum(v)}"
tot=[ (c['system_tokens'] or 0)+(c['task_tokens'] or 0) for m,c,d in lf]
print('accounted prompt tokens/run',q(tot))
scr=[sum(a['attributed_tokens'] for a in c['assets'] if a['kind']=='scratch') for m,c,d in lf]
print('scratch tokens/run',q(scr)); print('runs with scratch',sum(1 for s in scr if s))
print('scratch file count/run',q([sum(1 for a in c['assets'] if a['kind']=='scratch') for m,c,d in lf]))
share=[s/t for s,t in zip(scr,tot) if t]
print('scratch share p50',round(statistics.median(share),2),'runs >50%',sum(1 for x in share if x>.5),'runs >80%',sum(1 for x in share if x>.8))
over=[(t,m['run_id'],m.get('skill'),wave(m),m['harness'],os.path.basename(m['worktree'] or '')) for (m,c,d),t in zip(lf,tot)]
print('top 8 largest prompts');
for x in sorted(over,reverse=True)[:8]: print(' ',x)
print('prompts >100k',sum(1 for t in tot if t>100000),'>200k',sum(1 for t in tot if t>200000))
# by skill
by=collections.defaultdict(lambda: collections.Counter()); n=collections.Counter()
for m,c,d in lf:
    k=m.get('skill') or '(none)'; n[k]+=1
    for a in c['assets']:
        by[k][a['kind']]+=a['attributed_tokens']
print('avg tokens per run by skill: total | scratch memory goal(step/task msg) skill operating')
for k,cnt in n.most_common(14):
    b=by[k]; t=sum(b.values())
    print(f" {k:18} n={cnt:3} total={t//cnt:7} scratch={b['scratch']//cnt:7} memory={b['memory']//cnt:6} goal={b['goal']//cnt:6} skill={b['skill_instructions']//cnt:5} op={b['operating_instructions']//cnt:5} doc={b['document']//cnt:5}")
# by wave
byw=collections.defaultdict(collections.Counter); nw=collections.Counter()
for m,c,d in lf:
    w=wave(m) or '(none)'; nw[w]+=1
    for a in c['assets']: byw[w][a['kind']]+=a['attributed_tokens']
for w,cnt in nw.most_common():
    b=byw[w]; t=sum(b.values()); print(f" wave {w:15} n={cnt:3} total/run={t//cnt:7} scratch={b['scratch']//cnt:7} memory={b['memory']//cnt:6}")
# scratch heavy paths
sp=collections.Counter(); spn=collections.Counter()
for m,c,d in lf:
    for a in c['assets']:
        if a['kind']=='scratch': sp[a['source_path']]+=a['attributed_tokens']; spn[a['source_path']]+=1
print('distinct scratch paths',len(sp))
for p,t in sp.most_common(12): print(f'  {t:>8} n={spn[p]:3} avg={t//spn[p]:6} {p}')
# parent vs child
idx={m['run_id']:(m,c) for m,c,d in lf}
same=diff=0; childtot=[]
for m,c,d in lf:
    p=m.get('parent_run_id')
    if p:
        childtot.append((c['system_tokens'] or 0)+(c['task_tokens'] or 0))
        if p in idx:
            pm,pc=idx[p]
            ck={a['content_sha256'] for a in c['assets'] if a['kind'] in('memory','scratch')}
            pk={a['content_sha256'] for a in pc['assets'] if a['kind'] in('memory','scratch')}
            if ck and ck<=pk: same+=1
            else: diff+=1
print('children',len(childtot),q(childtot) if childtot else '', 'child memory+scratch subset of parent',same,'not',diff)
ck=collections.Counter()
for m,c,d in lf:
    if m.get('parent_run_id'):
        ck[(m.get('skill'),any(a['kind']=='memory' for a in c['assets']),any(a['kind']=='scratch' for a in c['assets']))]+=1
print('children (skill,memory,scratch)',ck.most_common(10))
