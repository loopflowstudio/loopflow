"""Summarize final timing and excluded profiles without publishing private rows."""
import hashlib
import json
from pathlib import Path
import re

from final_measure import ROOT, COMMITS

OUT = Path(__file__).parent
FUNCTIONS = ('read_claude_keychain_blob_with_security', 'LaunchCapacity', 'check_scope',
             'session_summary', 'sqlite3_step', 'RepoId8discover', 'sync_scope',
             'deliver_fields', 'resolve_repository_context', 'resolve_layers')


def stacks():
    result = {}
    for side in COMMITS:
        text = (ROOT / f'sample-{side}.txt').read_text()
        main = text.split('Call graph:\n',1)[1].splitlines()
        total = int(re.search(r'\d+',main[0]).group())
        lines = []
        for line in main[1:]:
            if re.match(r'^    \d+ Thread_',line) or line.startswith('Total number'):
                break
            match = re.match(r'^([ +!:|`]+)(\d+) (.+)',line)
            if match: lines.append((len(match[1]),int(match[2]),match[3]))
        functions = {}
        for name in (*FUNCTIONS, 'capacity_scope_sql_union'):
            count, parent = 0, None
            for depth,n,symbol in lines:
                if parent is not None and depth > parent: continue
                parent = None
                if (any(term in symbol for term in ('LaunchCapacity', 'check_scope', 'sqlite3_step'))
                    if name == 'capacity_scope_sql_union' else name in symbol):
                    count += n
                    parent = depth
            functions[name] = dict(samples=count, percent=100*count/total)
        receipt = json.loads((ROOT / f'sample-{side}-receipt.json').read_text())
        result[side] = dict(main_thread_samples=total, functions=functions, receipt=receipt)
    return result


def sql_profiles():
    result = []
    for read in json.loads((ROOT / 'profiles.json').read_text()):
        records = [json.loads(l) for l in (ROOT / (read['label']+'-trace.jsonl')).read_text().splitlines()]
        groups = {}
        for row in records:
            group = groups.setdefault(row['sql'], dict(sql=row['sql'], plan=row['plan'], plan_error=row['plan_error'],
                executions=0, scan=0, vm=0, ms=0, sort=0))
            group['executions'] += 1
            for key in ('scan','vm','sort'): group[key] += row[key]
            group['ms'] += row['ns']/1e6
        top = sorted(groups.values(),key=lambda x:x['ms'],reverse=True)[:10]
        targeted = []
        target_shapes = {}
        for row in records:
            query = row['sql']
            family = ('live-agent' if 'e.attached_lf_process_id,e.attachment_token' in query and 'ORDER BY e.id' in query
                      else 'task-process-membership' if 'SELECT ae.id FROM' in query and 'SELECT se.lf_process_id' in query
                      else 'session-summary' if query.startswith('WITH page AS MATERIALIZED') else None)
            if family:
                key = hashlib.sha256(query.encode()).hexdigest()[:16]
                target_shapes[key] = dict(sql=query, plan=row['plan'])
                targeted.append(dict(family=family, shape=key, ms=row['ns']/1e6,
                    scan=row['scan'], vm=row['vm']))
        result.append(dict(**read, sqlite_version=records[0]['sqlite_version'], top_sql=top, target_shapes=target_shapes, targeted_executions=targeted,
            transaction_statements=[r['sql'] for r in records if re.match(r'\s*BEGIN\b',r['sql'],re.I)],
            write_lock_probes=[dict(ms=r['ns']/1e6, following_statements=[n['sql'] for n in records[i+1:i+3]])
                for i,r in enumerate(records) if r['sql']=='BEGIN IMMEDIATE']))
    return result


def main():
    result = json.loads((ROOT / 'compare-checks.json').read_text())
    assert result['errors'] == 0 and all(result['sealed_checks'].values())
    for workload,comparison in result['comparisons'].items():
        assert not comparison['full_json_differences'] and not comparison['warmup_mismatches'], workload
        b,a = (comparison['summary'][s] for s in COMMITS)
        assert b['n']==a['n']==5
        comparison['reduction_pct'] = 100*(1-a['median_ms']/b['median_ms'])
        comparison['verdict'] = ('better' if a['range_ms'][1]<b['range_ms'][0] else
                                 'worse' if b['range_ms'][1]<a['range_ms'][0] else 'no measurable difference')
    all_rows = [json.loads(l) for l in (ROOT / 'trials.jsonl').read_text().splitlines()]
    result['excluded_trials'] = [r for r in all_rows if not r['label'].startswith('compare-')]
    result['manifest'] = json.loads((ROOT / 'manifest.json').read_text())
    profiles = dict(stacks=stacks(), sql=sql_profiles())
    expected = {w: info['summary']['before']['full_digests'][0] for w,info in result['comparisons'].items()}
    receipts = [s['receipt'] for s in profiles['stacks'].values()] + profiles['sql']
    for read in receipts:
        assert read['errors']==0
        assert read['full_digest']==read['warmup']['full_digest']==expected[read['workload']], read['label']
    result['profile_validation'] = dict(reads=len(receipts), errors=0, all_full_json_matches=True)
    result['task_session_checks'] = json.loads((ROOT / 'task-session-checks.json').read_text())
    (OUT / 'final-results.json').write_text(json.dumps(result,indent=2)+'\n')
    (OUT / 'final-profiles.json').write_text(json.dumps(profiles,indent=2)+'\n')
    for workload,comparison in result['comparisons'].items():
        print(workload,comparison['verdict'],comparison['summary'])
    for side,stack in profiles['stacks'].items():
        print(side,stack['main_thread_samples'],{k:round(v['percent'],1) for k,v in stack['functions'].items()})
    for read in profiles['sql']:
        print(read['side'],read['workload'],round(read['ms'],1),[(round(q['ms'],1),q['executions'],q['scan'],q['vm'],q['sql'][:60]) for q in read['top_sql'][:3]])


if __name__ == '__main__': main()
