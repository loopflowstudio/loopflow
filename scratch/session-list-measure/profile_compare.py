"""Separate monotonic SQLite attribution runs, with actual bound EXPLAIN plans."""
import json
from pathlib import Path
import compare as c
from planning_compare import cli as offline_cli

if __name__=='__main__':
    results=[]
    normal_cli=c.cli
    for side in ('before','after1','after2'):
        for workload in ('cli','served','task','waves','board','selection'):
            label=f'profile-{side}-{workload}'
            c.activate(side,True)
            env=c.environment()
            c.cli=normal_cli if workload in ('cli','served') else offline_cli
            if workload!='served':
                _,data,_=c.cli(workload,env);c.validate(workload,data)
            trace=c.ROOT/(label+'-trace.jsonl');trace.unlink(missing_ok=True)
            env['SESSION_LIST_TRACE']=str(trace)
            ms,data,extra=c.served(env,label) if workload=='served' else c.cli(workload,env)
            row=dict(side=side,workload=workload,label=label,errors=0,ms=ms,**c.validate(workload,data))
            # Invalid records reject a trace; partial counters are never used.
            records=[json.loads(line) for line in trace.read_text().splitlines()]
            assert all(r['ns'] is not None for r in records)
            row['statements']=len(records)
            (c.ROOT/(label+'.json')).write_text(json.dumps(data))
            results.append(row);print(json.dumps(row),flush=True)
            (c.ROOT/'profiles.json').write_text(json.dumps(results,indent=2))
