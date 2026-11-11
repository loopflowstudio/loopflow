"""Private-store wall baseline. Read outputs inside timing; inspect values afterwards."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import statistics
import subprocess
import time

ROOT = Path('/tmp/loo822-measure')
REPO = Path(__file__).resolve().parents[2]


def environment(home: Path) -> dict:
    env = {k:v for k,v in os.environ.items() if not k.startswith('LF_')}
    env.update(LF_HOME=str(home), LF_BIN=str(ROOT/'lf'), LF_NO_UPDATE_CHECK='1')
    return env


def check(rows: list, db: Path) -> dict:
    if not isinstance(rows, list) or len({r['id'] for r in rows}) != len(rows):
        raise ValueError('invalid or duplicate rows')
    with sqlite3.connect(f'file:{db}?mode=ro', uri=True) as conn:
        for r in rows:
            if not r['raw']:
                stored = conn.execute('SELECT title,cwd FROM agent_sessions WHERE id=?', (r['id'],)).fetchone()
                if stored is None or stored[0] != r['title'] or stored[1] != r['cwd']:
                    raise ValueError('row differs from store')
            if r['turn']:
                if r['turn']['state'] not in ('running','ended','interrupted'):
                    raise ValueError('invalid turn')
                if r['turn']['usage'] and any(n < 0 for n in r['turn']['usage'].values()):
                    raise ValueError('negative usage')
    normalized = sorted((r['id'], r['title'], r['cwd'], r['raw']) for r in rows)
    return dict(rows=len(rows), raw=sum(r['raw'] for r in rows), turns=sum(r['turn'] is not None for r in rows), digest=hashlib.sha256(json.dumps(normalized).encode()).hexdigest(), full_digest=hashlib.sha256(json.dumps(rows,sort_keys=True).encode()).hexdigest())


def run(label: str, count: int, home: Path, extra: list[str]) -> None:
    results = []
    for i in range(count):
        t = time.perf_counter()
        p = subprocess.run([str(ROOT/'lf'), 'session','list','--json',*extra],cwd=REPO,env=environment(home),stdin=subprocess.DEVNULL,capture_output=True,timeout=120)
        ms=(time.perf_counter()-t)*1000
        row=dict(label=label, run=i, ms=ms, errors=int(p.returncode!=0))
        (ROOT/f'{label}-{i}.stderr').write_bytes(p.stderr)
        if p.returncode == 0:
            data=json.loads(p.stdout)
            try: row.update(check(data, home/'loopflow.db'))
            except Exception as e: row.update(errors=1,error=str(e))
            (ROOT/f'{label}-{i}.json').write_bytes(p.stdout)
        results.append(row)
        print(json.dumps(row),flush=True)
        (ROOT/f'{label}-results.json').write_text(json.dumps(results,indent=2))
    values=[r['ms'] for r in results if not r['errors']]
    print(json.dumps(dict(label=label,n=len(values),median_ms=statistics.median(values) if values else None,range_ms=[min(values),max(values)] if values else None, errors=sum(r['errors'] for r in results))),flush=True)

if __name__ == '__main__':
    p=argparse.ArgumentParser();p.add_argument('label');p.add_argument('--count',type=int,default=5);p.add_argument('--home',type=Path,default=ROOT/'home');p.add_argument('extra',nargs='*');a=p.parse_args();run(a.label,a.count,a.home,a.extra)
