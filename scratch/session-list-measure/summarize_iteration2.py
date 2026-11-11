"""Retain every iteration-2 trial and the targeted bundled-SQLite counters."""
import hashlib
import json
from pathlib import Path
import statistics
from iteration2 import ROOT, SIDES, WORKLOADS

OUT = Path(__file__).parent
rows = [json.loads(line) for line in (ROOT / 'trials.jsonl').read_text().splitlines()]
cohorts = {}
for workload in WORKLOADS:
    sides = {}
    for side in SIDES:
        samples = [r for r in rows if r['label'].startswith('iteration2-') and r['workload'] == workload and r['side'] == side]
        assert len(samples) == 5 and all(r['errors'] == 0 for r in samples)
        values = [r['ms'] for r in samples]
        sides[side] = dict(n=5, median_ms=statistics.median(values), range_ms=[min(values), max(values)],
                           rows=samples[0]['rows'], errors=0, full_digest=samples[0]['full_digest'])
    assert len({r['full_digest'] for r in sides.values()}) == 1
    b, a = sides['before'], sides['after']
    verdict = ('better' if a['range_ms'][1] < b['range_ms'][0] else
               'worse' if b['range_ms'][1] < a['range_ms'][0] else 'no measurable difference')
    timing_verdict = verdict
    if workload == 'board' and verdict == 'better':
        # This iteration's bound plans use neither new index; all substantive
        # statement plans and work counters match. The small gap is unexplained.
        verdict = 'inconclusive'
    cohorts[workload] = dict(sides=sides, verdict=verdict, timing_verdict=timing_verdict, reduction_pct=100*(1-a['median_ms']/b['median_ms']))
    print(workload, verdict, [(s, round(v['median_ms'], 1), [round(n, 1) for n in v['range_ms']]) for s, v in sides.items()])
excluded = [json.loads(line) for line in (ROOT / 'unsealed/trials.jsonl').read_text().splitlines()]
result = dict(manifest=json.loads((ROOT / 'manifest.json').read_text()), cohorts=cohorts, trials=rows,
              excluded_cohort=dict(reason='Unsealed baseline copy omitted its WAL and the already-kept Task indexes; never use these samples for a speedup claim.', trials=excluded))
(OUT / 'iteration2-results.json').write_text(json.dumps(result, indent=2)+'\n')

if (ROOT / 'profiles.json').exists():
    profiles = []
    for read in json.loads((ROOT / 'profiles.json').read_text()):
        records = [json.loads(line) for line in (ROOT / (read['label']+'-trace.jsonl')).read_text().splitlines()]
        shapes = {}
        for record in records:
            sql = record['sql']
            if 'e.attached_lf_process_id,e.attachment_token' in sql and 'ORDER BY e.id' in sql:
                family = 'live-agent'
            elif 'SELECT ae.id FROM' in sql and 'SELECT se.lf_process_id' in sql:
                family = 'task-process-membership'
            elif sql.startswith('WITH page AS MATERIALIZED'):
                family = 'session-summary'
            else:
                family = None
            key = sql
            group = shapes.setdefault(key, dict(family=family, sql=sql, plan=record['plan'], plan_error=record['plan_error'],
                                                executions=0, scan=0, vm=0, sort=0, ms=0, per_execution=[]))
            group['executions'] += 1
            for field in ('scan', 'vm', 'sort'):
                group[field] += record[field]
            group['ms'] += record['ns']/1e6
            group['per_execution'].append(dict(ms=record['ns']/1e6, **{f: record[f] for f in ('scan', 'vm', 'sort')}))
        selected = [v for v in shapes.values() if v['family']]
        top = sorted(shapes.values(), key=lambda v: v['ms'], reverse=True)[:8]
        profiles.append(dict(**read, sqlite_version=records[0]['sqlite_version'], queries=selected, top_sql=top))
        print(read['side'], read['workload'], round(read['ms'], 1),
              [(q['family'], q['executions'], q['scan'], q['vm'], round(q['ms'], 1)) for q in selected])
    shapes = {}
    for read in profiles:
        queries = {}
        top = []
        for query in [*read['queries'], *read['top_sql']]:
            shape = {k: query[k] for k in ('family', 'sql', 'plan', 'plan_error')}
            key = hashlib.sha256(json.dumps(shape, sort_keys=True).encode()).hexdigest()[:16]
            shapes[key] = shape
            counters = {k: v for k, v in query.items() if k not in shape}
            if shape['family'] is None:
                executions = counters.pop('per_execution')
                counters['profile_ms_range'] = [min(e['ms'] for e in executions), max(e['ms'] for e in executions)]
            queries[key] = dict(shape=key, **counters)
        for query in read['top_sql']:
            shape = {k: query[k] for k in ('family', 'sql', 'plan', 'plan_error')}
            top.append(hashlib.sha256(json.dumps(shape, sort_keys=True).encode()).hexdigest()[:16])
        read['queries'] = list(queries.values())
        read['top_sql'] = top
    (OUT / 'iteration2-profiles.json').write_text(json.dumps(dict(shapes=shapes, reads=profiles), indent=2)+'\n')
