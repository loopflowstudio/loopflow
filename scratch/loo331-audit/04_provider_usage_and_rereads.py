import json,os,collections,statistics,re
runs=json.load(open('runs.json'))
lf=[r for r in runs if r[0]['repo'].startswith('/Users/jack/src/loopflow')]
def wave(m):
    for s in m.get('subjects') or []:
        if s['selector'].startswith('wave:'): return s['selector'][5:]
rows=[]
noev=nous=0
reread_mem=collections.Counter(); reread_scr=0; release_read=[];
for m,c,d in lf:
    p=os.path.join(d,'events.jsonl')
    if not os.path.exists(p): noev+=1; continue
    first=None; last=None; streams=set()
    acc=(c['system_tokens'] or 0)+(c['task_tokens'] or 0)
    w=wave(m); hasmem=any(a['kind']=='memory' for a in c['assets'])
    scr={a['source_path'] for a in c['assets'] if a['kind']=='scratch'}
    memhit=False; scrhit=False; relhit=False
    for line in open(p):
        if '"type":"usage"' in line:
            e=json.loads(line); u=e['usage']; streams.add(e['usage_stream_id'])
            if first is None: first=u
            last=u
        elif '"item_started"' in line and '"command"' in line:
            try: e=json.loads(line)
            except Exception: continue
            cmd=' '.join(e['event']['item'].get('command') or [])
            if w and f'wave/{w}/MEMORY.md' in cmd and re.search(r'\b(cat|sed -n|head|nl|rg|grep|wc)\b',cmd) and '>' not in cmd.split('MEMORY.md')[0][-3:]: memhit=True
            if 'wave/infrastructure/release' in cmd: relhit=True
            if any(s in cmd for s in scr) and re.search(r'\b(cat|sed -n|nl)\b',cmd): scrhit=True
    if relhit: release_read.append((m['created_at'][:16],m['run_id'],m.get('skill'),w,os.path.basename(m['worktree'] or '')))
    if hasmem: reread_mem[memhit]+=1
    if scr and scrhit: reread_scr+=1
    if first is None: nous+=1; continue
    rows.append(dict(id=m['run_id'],h=m['harness'],acc=acc,first=first.get('total_input_tokens'),last=last.get('total_input_tokens'),peak=last.get('peak_input_tokens'),win=last.get('context_window_tokens'),streams=len(streams),out=last.get('output_tokens'),skill=m.get('skill'),parent=bool(m.get('parent_run_id'))))
print('runs',len(lf),'no events',noev,'no usage observation',nous,'with usage',len(rows))
def q(v):
    v=sorted(x for x in v if x is not None); n=len(v)
    return f"n={n} min={v[0]} p50={v[n//2]} p90={v[int(n*.9)]} max={v[-1]}" if n else 'n=0'
for h in ('codex','claude'):
    r=[x for x in rows if x['h']==h and x['first'] is not None and x['streams']==1]
    print(h,'first-observation total_input',q([x['first'] for x in r]))
    print(h,'first-observation minus accounted (unattributed provider-side input; first obs may span >1 request)',q([x['first']-x['acc'] for x in r]))
    print(h,'accounted/first share',q([round(100*x['acc']/x['first']) for x in r if x['first']]))
    print(h,'final cumulative total_input / accounted',q([round(x['last']/x['acc'],1) for x in r if x['last'] and x['acc']]))
    print(h,'peak/window %',q([round(100*x['peak']/x['win']) for x in r if x['peak'] and x['win']]))
# association: accounted size vs cumulative input
import math
r=[x for x in rows if x['last'] and x['acc'] and x['streams']==1]
xs=[x['acc'] for x in r]; ys=[x['last'] for x in r]
mx=sum(xs)/len(xs); my=sum(ys)/len(ys)
cov=sum((a-mx)*(b-my) for a,b in zip(xs,ys));
print('pearson accounted vs cumulative input',round(cov/math.sqrt(sum((a-mx)**2 for a in xs)*sum((b-my)**2 for b in ys)),2),'n',len(r))
print('memory included; agent also read its wave MEMORY.md by command:',reread_mem)
print('runs that read an already-included scratch file by command:',reread_scr)
print('runs touching wave/infrastructure/release by command:',len(release_read))
for x in sorted(release_read): print('  ',x)
