"""Second baseline: count every commit, including session_activity updates."""
import json,sqlite3,time
from run import ROOT,run

c=sqlite3.connect(f'file:{ROOT}/home/loopflow.db?mode=ro',uri=True)
def snapshot():
    return dict(data_version=c.execute('PRAGMA data_version').fetchone()[0], revisions=dict(c.execute('SELECT domain,revision FROM store_revisions')), events=c.execute('SELECT count(*) FROM session_events').fetchone()[0])
results=[]
for i in range(5):
    for side in (['quiet','writing'] if i%2==0 else ['writing','quiet']):
        before=snapshot()
        run(f'{side}-{i}',1,ROOT/('control' if side=='quiet' else 'home'),[])
        after=snapshot()
        r=dict(pair=i,side=side,before=before,after=after)
        results.append(r);print(json.dumps(r),flush=True)
        (ROOT/'commit-windows.json').write_text(json.dumps(results,indent=2))
    # Wait for the next natural provider commit, never inject a write.
    deadline=time.monotonic()+10
    while snapshot()['data_version']==after['data_version'] and time.monotonic()<deadline:time.sleep(.1)
