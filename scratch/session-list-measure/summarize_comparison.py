"""Retain measured samples and aggregate the four named SQLite query families."""
import collections
import hashlib
import json
from pathlib import Path
import re
import statistics

ROOT=Path('/tmp/loo822-compare')
OUT=Path(__file__).parent
PATTERN=re.compile(r'^(task-probes|smaller-fixes|combined|planning-increment|planning-offline)-(\w+)-(\d+)-(before|after1|after2)$')
rows=[json.loads(line) for line in (ROOT/'trials.jsonl').read_text().splitlines()]
groups={}
for row in rows:
    match=PATTERN.fullmatch(row['label'])
    if not match:continue
    comparison,workload,pair,side=match.groups()
    groups.setdefault(comparison,{}).setdefault(workload,{}).setdefault(side,[]).append(row)
cohorts={}
for comparison,workloads in groups.items():
    cohorts[comparison]={}
    for workload,sides in workloads.items():
        cohorts[comparison][workload]={}
        for side,samples in sides.items():
            assert len(samples)==5 and all(r['errors']==0 for r in samples)
            values=[r['ms'] for r in samples]
            cohorts[comparison][workload][side]=dict(n=5,median_ms=statistics.median(values),range_ms=[min(values),max(values)],errors=0,rows=samples[0]['rows'],full_digest=samples[0]['full_digest'],samples=samples)
        assert len({s['full_digest'] for s in cohorts[comparison][workload].values()})==1
receipt=dict(manifest=json.loads((ROOT/'manifest.json').read_text()),cohorts=cohorts,
    other_trials=[r for r in rows if not PATTERN.fullmatch(r['label'])],
    incomplete_trials=[dict(label='planning-task-0-after2',status='interrupted',ms=None,errors=None,reason='Task status starts a planning exchange; stopped this cohort and began a network-denied baseline.')],
    rejected_traces=dict(reason='Initial collector interleaved 72 task-trace records and SQLite PROFILE elapsed used frozen wall time; none supplies final attribution.',paths=['trace-after1-*','after2-trace-old']))
(OUT/'comparison-results.json').write_text(json.dumps(receipt,indent=2)+'\n')
for comparison,workloads in cohorts.items():
 for workload,sides in workloads.items():
  print(comparison,workload,[(s,round(v['median_ms'],2),[round(x,2) for x in v['range_ms']]) for s,v in sides.items()])


def family(sql):
 if sql.startswith('WITH page AS MATERIALIZED'):
  return 'Q112' if 'WHERE s.id=?1)' in sql else 'Q102'
 if sql.startswith('SELECT id FROM agent_sessions WHERE provider_thread='):return 'Q111'
 if sql.startswith('SELECT MIN(observed_at'):return 'Q114'
 return None

if (ROOT/'profiles.json').exists():
 traces=[]
 for profile in json.loads((ROOT/'profiles.json').read_text()):
  records=[json.loads(line) for line in (ROOT/(profile['label']+'-trace.jsonl')).read_text().splitlines()]
  shapes={}
  for record in records:
   sql=record['sql']; key=re.sub(r'\b1794\d{6}\b','<NOW>',sql)
   group=shapes.setdefault(key,dict(family=family(sql),sql=sql,plan=record['plan'],plan_error=record['plan_error'],executions=0,fullscan_steps=0,vm_steps=0,sorts=0,profile_ms=0,per_execution=[]))
   group['executions']+=1
   for field,source in [('fullscan_steps','scan'),('vm_steps','vm'),('sorts','sort')]:group[field]+=record[source]
   group['profile_ms']+=record['ns']/1e6
   group['per_execution'].append(dict(vm=record['vm'],scan=record['scan'],sort=record['sort'],ms=record['ns']/1e6))
  selected=[v for v in shapes.values() if v['family'] or v['sql'].startswith('SELECT t.id,t.external_issue_id') or (v['plan'] and any('idx_tasks_repository' in x for x in v['plan']))]
  # Planning's top SQL is retained too, including statements outside the candidate.
  top=sorted(shapes.values(),key=lambda x:x['profile_ms'],reverse=True)[:5]
  traces.append(dict(**profile,sqlite_version=records[0]['sqlite_version'],queries=selected,top_sql=top,total_statements=len(records),non_readonly_statements=sum(not r['readonly'] for r in records)))
 # Store each SQL/plan once; keep every requested counter and trace reading.
 shapes={}
 for trace in traces:
  counters={}
  top=[]
  for query in [*trace['queries'],*trace['top_sql']]:
   shape={k:query[k] for k in ('family','sql','plan','plan_error')}
   key=hashlib.sha256(json.dumps(shape,sort_keys=True).encode()).hexdigest()[:16]
   shapes[key]=shape
   value={k:v for k,v in query.items() if k not in shape}
   if not shape['family']:
    samples=value.pop('per_execution')
    value['profile_ms_range']=[min(x['ms'] for x in samples),max(x['ms'] for x in samples)]
   counters[key]=dict(shape=key,**value)
  for query in trace['top_sql']:
   shape={k:query[k] for k in ('family','sql','plan','plan_error')}
   top.append(hashlib.sha256(json.dumps(shape,sort_keys=True).encode()).hexdigest()[:16])
  trace['queries']=list(counters.values());trace['top_sql']=top
 (OUT/'comparison-profiles.json').write_text(json.dumps(dict(builds=json.loads((ROOT/'trace-manifest.json').read_text()),shapes=shapes,reads=traces),indent=2)+'\n')
