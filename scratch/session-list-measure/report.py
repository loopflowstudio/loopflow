"""Retain SQL shapes/plans and compact receipts; never retain row values."""
import hashlib,json,re,statistics,subprocess
from pathlib import Path
ROOT=Path('/tmp/loo822-measure')
OUT=Path(__file__).resolve().parent
shapes={}
for cohort in ['bare','served','once']:
    for r in map(json.loads,(ROOT/f'{cohort}-trace.jsonl').read_text().splitlines()):
        key=re.sub(r'\b1794\d{6}\b','<NOW>',r['sql'])
        entry=shapes.setdefault(key,dict(sql=r['sql'],plan=r['plan'],cohorts={}))
        g=entry['cohorts'].setdefault(cohort,dict(executions=0,inclusive_ms=0,fullscan_steps=0,vm_steps=0,sorts=0))
        g['executions']+=1;g['inclusive_ms']+=r['ns']/1e6;g['fullscan_steps']+=r['scan'];g['vm_steps']+=r['vm'];g['sorts']+=r['sort']
for r in json.loads((ROOT/'opencode-plans.json').read_text()):
    if r['sql'].startswith('SELECT id, time_updated'): continue
    p=subprocess.run([str(ROOT/'explain'),str(ROOT/'opencode.db'),'EXPLAIN QUERY PLAN '+r['sql']],check=True,capture_output=True,text=True)
    shapes[r['sql']]=dict(sql=r['sql'],plan=p.stdout.splitlines()[1:],cohorts={},note='Supplemental OpenCode plan; same SQLite 3.53.2 amalgamation, read-only store copy, not reached by these scoped listings.')
source=(Path('rust/loopflow/src/store/sqlite.rs')).read_text()
columns=re.search(r'const PROVIDER_ACCOUNT_COLUMNS: &str = "([^"]+)";',source).group(1)
sql=f'SELECT {columns} FROM provider_accounts WHERE provider = ?1 AND account_id = ?2'
p=subprocess.run([str(ROOT/'explain'),str(ROOT/'home/loopflow.db'),'EXPLAIN QUERY PLAN '+sql],check=True,capture_output=True,text=True)
shapes[sql]=dict(sql=sql,plan=p.stdout.splitlines()[1:],cohorts={},note='Conditional legacy isolated-account lookup; same SQLite 3.53.2 amalgamation, not reached in listings. Config preferences are filesystem reads.')
lines=['# Executed SQL and query plans','', 'Captured from the release branch with a temporary SQLite PROFILE callback. Query plans use the same connection and SQLite 3.53.2, with NULL placeholder bindings; no bound values or result content are retained. Timestamp literals are grouped across readings. `inclusive_ms` includes nested reads and profiler overhead and must not be added to obtain wall time. Zero milliseconds means below SQLite’s millisecond profile resolution. PRAGMA/control statements legitimately have empty query plans.','']
for i,e in enumerate(shapes.values(),1):
 e['id']=f'Q{i:03d}'
 lines.extend([f'## {e["id"]}','','```sql',e['sql'],'```','','```text',*e['plan'],'```' if e['plan'] else '# no relational plan\n```',''])
 if e['cohorts']:
  lines.extend(['| Run | Executions | Inclusive ms | Fullscan steps | VM steps | Sorts |','|---|---:|---:|---:|---:|---:|'])
  for c,g in e['cohorts'].items(): lines.append(f'| {c} | {g["executions"]} | {g["inclusive_ms"]:.0f} | {g["fullscan_steps"]} | {g["vm_steps"]} | {g["sorts"]} |')
 else:lines.append(e['note'])
 lines.append('')
(OUT/'query-plans.md').write_text('\n'.join(lines))
(OUT/'queries.json').write_text(json.dumps(list(shapes.values()),indent=2))
cohorts={}
for name in ['idle','easy','control','live','quiet','writing']:
 rows=[json.loads((ROOT/f'{name}-{i}-results.json').read_text())[0] for i in range(5)]
 a=[r['ms'] for r in rows];cohorts[name]=dict(samples=rows,median_ms=statistics.median(a),min_ms=min(a),max_ms=max(a),errors=sum(r['errors'] for r in rows))
rows=json.loads((ROOT/'served-results.json').read_text()); a=[r['ms'] for r in rows[1:]]
cohorts['served']=dict(samples=rows,median_ms=statistics.median(a),min_ms=min(a),max_ms=max(a),errors=0,first_excluded=True)
receipt=dict(commit='00851d9c6de7ae2beb8779674e2a482fde19269d',binary_sha256=hashlib.sha256((ROOT/'lf').read_bytes()).hexdigest(),cohorts=cohorts,writer_windows=json.loads((ROOT/'commit-windows.json').read_text()),store_counts=json.loads((ROOT/'counts.json').read_text()))
(OUT/'results.json').write_text(json.dumps(receipt,indent=2))
print('query shapes',len(shapes))
for e in shapes.values():
 if not e['sql'].lstrip().startswith('PRAGMA'):print(e['id'],e['sql'][:85].replace('\n',' '))
