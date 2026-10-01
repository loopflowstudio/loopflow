import json,os,glob,collections,statistics,sys
H=os.environ['LF_HOME']
runs=[]
missing_ctx=0; bad=0
for mp in glob.glob(f'{H}/runs/*/*/manifest.json'):
    d=os.path.dirname(mp)
    try: m=json.load(open(mp))
    except Exception: bad+=1; continue
    cp=os.path.join(d,'context.json')
    if not os.path.exists(cp): missing_ctx+=1; runs.append((m,None,d)); continue
    try: c=json.load(open(cp))['context']
    except Exception: bad+=1; continue
    runs.append((m,c,d))
print('manifests',len(runs),'without context',missing_ctx,'unreadable',bad)
ts=sorted(m['created_at'] for m,_,_ in runs)
print('window',ts[0],'->',ts[-1])
withc=[(m,c,d) for m,c,d in runs if c]
print('coverage',collections.Counter(c['coverage'] for _,c,_ in withc))
print('tokenizer',collections.Counter(c['tokenizer'] for _,c,_ in withc))
print('harness',collections.Counter(m['harness'] for m,_,_ in withc))
print('surface',collections.Counter(m['surface'] for m,_,_ in withc))
def wave(m):
    for s in m.get('subjects') or []:
        if s['selector'].startswith('wave:'): return s['selector'][5:]
    return None
def has(m,k):
    return any(s['selector'].startswith(k+':') for s in m.get('subjects') or [])
print('wave',collections.Counter(wave(m) for m,_,_ in withc))
print('skill',collections.Counter(m.get('skill') for m,_,_ in withc).most_common(25))
print('parented',sum(1 for m,_,_ in withc if m.get('parent_run_id')),'task-subject',sum(1 for m,_,_ in withc if has(m,'task')))
print('flow kinds',collections.Counter((m.get('flow') or {}).get('kind') for m,_,_ in withc))
dec=collections.Counter(); reasons=collections.Counter()
kinds=collections.Counter(); ktok=collections.Counter()
for m,c,d in withc:
    for x in c['decisions']:
        dec[x['decision']]+=1
        if x['decision']!='included': reasons[(x['kind'],x['reason'])]+=1
    for ch in ('system','task'):
        for a in (c[ch] or {}).get('assets',[]):
            key=(a['channel'],a['kind'],a['scope'],a['included_by'])
            kinds[key]+=1; ktok[key]+=a['attributed_tokens']
print('decisions',dec); print('non-included reasons',reasons.most_common(20))
tot=sum(ktok.values())
print('TOTAL attributed tokens',tot)
for k,v in ktok.most_common():
    print(f'{v:>9} {100*v/tot:5.1f}% n={kinds[k]:>4} avg={v//kinds[k]:>6}', k)
json.dump([ (m,{'system_tokens':(c['system'] or {}).get('tokens'),'task_tokens':(c['task'] or {}).get('tokens'),'assets':(c['system'] or {}).get('assets',[])+(c['task'] or {}).get('assets',[]),'decisions':c['decisions']} if c else None,d) for m,c,d in runs], open('/tmp/loo331/runs.json','w'))
