import json,os,collections,statistics,hashlib
runs=json.load(open('runs.json'))
H=os.environ['LF_HOME']
def wave(m):
    for s in m.get('subjects') or []:
        if s['selector'].startswith('wave:'): return s['selector'][5:]
lf=[r for r in runs if r[0]['repo'].startswith('/Users/jack/src/loopflow')]
print('loopflow-repo runs',len(lf),'other repos',collections.Counter(r[0]['repo'] for r in runs if r not in lf))
# memory assets
mem=collections.Counter(); memtok=collections.defaultdict(list)
inherit=0; rel=0
for m,c,d in lf:
    full=json.load(open(os.path.join(d,'context.json')))['context']
    text=(full['task'] or {}).get('text','')
    tb=text.encode()
    for a in c['assets']:
        if a['kind']=='memory':
            mem[(wave(m),a['source_path'])]+=1; memtok[a['source_path']].append(a['attributed_tokens'])
            seg=tb[a['byte_start']:a['byte_end']].decode(errors='replace')
            if '## Memory inherited from' in seg or '## Memory owned by' in seg: inherit+=1
        if a.get('source_path') and 'release' in a['source_path'] and 'wave/' in a['source_path']: rel+=1; print('REL',m['run_id'],a['kind'],a['source_path'],a['included_by'])
    if 'wave/infrastructure/release' in text: print('mentions release path',m['run_id'],m.get('skill'),wave(m))
print('memory assets by (wave,path)',mem)
for k,v in memtok.items(): print(k,'n',len(v),'min',min(v),'median',int(statistics.median(v)),'max',max(v))
print('runs with inherited/owned headers',inherit,'assets from wave/**/release',rel)
# wave subject but no memory / no wave but memory
nomem=collections.Counter(); 
for m,c,d in lf:
    hasmem=any(a['kind']=='memory' for a in c['assets'])
    hasgoal=any(a['kind']=='goal' and a['scope']=='wave' for a in c['assets'])
    nomem[(wave(m) is not None,hasmem,hasgoal)]+=1
print('(wave subject, memory asset, wave goal asset)',nomem)
# duplicates within run
dup_runs=0; dup_tok=0; dupkinds=collections.Counter(); goal_double=0; goal_double_tok=0
for m,c,d in lf:
    seen={}; hit=False
    paths=collections.Counter()
    for a in c['assets']:
        if a['kind']=='assembly': continue
        h=a['content_sha256']
        if h in seen:
            hit=True; dup_tok+=a['attributed_tokens']; dupkinds[(seen[h]['kind'],a['kind'],os.path.basename(a.get('source_path') or a['label']))]+=1
        else: seen[h]=a
        if a.get('source_path'): paths[a['source_path']]+=1
    for p,n in paths.items():
        if n>1 and p.endswith('GOAL.md'):
            goal_double+=1; goal_double_tok+=min(a['attributed_tokens'] for a in c['assets'] if a.get('source_path')==p)
    dup_runs+=hit
print('runs with identical-hash duplicate assets',dup_runs,'dup tokens',dup_tok, dupkinds.most_common(10))
print('runs with GOAL.md included twice (objective + full doc)',goal_double,'overlap tokens approx',goal_double_tok)
