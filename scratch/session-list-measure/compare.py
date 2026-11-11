"""Frozen private-store comparison; raw answers remain in the private directory."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import sqlite3
import subprocess
import time

from run import check

ROOT = Path('/tmp/loo822-compare')
REPO = Path(__file__).resolve().parents[2]
HOME = ROOT / 'home'
BIN = ROOT / 'lf'
COMMANDS = {
    'cli': ['session', 'list', '--json'],
    'easy': ['session', 'list', '--limit', '1', '--json'],
    'task': ['task', 'status', 'LOO-822', '--json'],
    'waves': ['wave', 'list', '--json'],
    'board': ['plan', 'board', '--json'],
    'selection': ['desktop', 'view', 'show', '--json'],
}


def environment():
    env = {k: v for k, v in os.environ.items() if not k.startswith('LF_')}
    env.update(LF_HOME=str(HOME), LF_BIN=str(BIN), LF_NO_UPDATE_CHECK='1',
               DYLD_INSERT_LIBRARIES=str(ROOT/'freeze_clock.dylib'),
               MEASURE_EPOCH=(ROOT/'epoch').read_text(),
               CLAUDE_CONFIG_DIR=str(ROOT/'providers/claude'),
               CODEX_HOME=str(ROOT/'providers/codex'),
               PI_CODING_AGENT_DIR=str(ROOT/'providers/pi'),
               XDG_DATA_HOME=str(ROOT/'providers/data'))
    return env


def activate(side, trace=False):
    (ROOT/'next-lf').unlink(missing_ok=True)
    os.link(ROOT/(side + ('-trace' if trace else '')), ROOT/'next-lf')
    os.replace(ROOT/'next-lf', BIN)
    for suffix in ('', '-wal', '-shm'):
        (HOME/('loopflow.db'+suffix)).unlink(missing_ok=True)
    subprocess.run(['cp', '-c', str(ROOT/(side+'.db')), str(HOME/'loopflow.db')], check=True)


def digest(data):
    return hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest()


def validate(workload, data):
    if workload in ('cli', 'easy', 'served'):
        result = check(data, HOME/'loopflow.db')
        if workload == 'cli': assert result['rows'] == 100
        if workload == 'easy': assert result['rows'] == 1
        assert result['rows'] > 0
        return result
    if workload == 'task':
        assert data['task']['identifier'] == 'LOO-822'
        count = 1
    elif workload == 'board':
        assert data['tasks']['state'] == 'ok'
        count = len(data['tasks']['items'])
        assert count > 0
    elif workload == 'selection':
        count = len(data['resolution']['members'])
        assert count > 0
    else:
        assert isinstance(data, list) and data
        count = len(data)
    if workload in ('task', 'board'):
        records = [data['task']] if workload == 'task' else data['tasks']['items']
        with sqlite3.connect(f'file:{HOME}/loopflow.db?mode=ro', uri=True) as conn:
            for record in records:
                stored = conn.execute('SELECT issue_title FROM tasks WHERE id=?', (record['id'],)).fetchone()
                assert stored is not None and stored[0] == record['title']
    return dict(rows=count, full_digest=digest(data))


def cli(workload, env):
    start = time.perf_counter()
    p = subprocess.run([str(BIN), *COMMANDS[workload]], cwd=REPO, env=env,
                       stdin=subprocess.DEVNULL, capture_output=True, timeout=120)
    ms = (time.perf_counter()-start)*1000
    if p.returncode:
        raise RuntimeError(f'command exit {p.returncode}: {p.stderr.decode()[:300]}')
    return ms, json.loads(p.stdout), p.stderr.decode()


def served(env, label):
    path = Path('/tmp') / f'lf-{os.getuid()}-{hashlib.sha256(str(HOME.resolve()).encode()).hexdigest()[:16]}' / 'machine.sock'
    path.unlink(missing_ok=True)
    with (ROOT/(label+'.log')).open('w') as log:
        proc = subprocess.Popen([str(BIN), 'server', 'run'], env=env, cwd=REPO,
                                stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            until = time.monotonic()+30
            while not path.exists():
                if proc.poll() is not None or time.monotonic()>until:
                    raise RuntimeError('server startup failed')
                time.sleep(.05)
            with socket.socket(socket.AF_UNIX) as sock:
                sock.settimeout(120); sock.connect(str(path)); f=sock.makefile('rwb')
                def send(m):
                    f.write(json.dumps(m).encode()+b'\n'); f.flush()
                def receive():
                    m=json.loads(f.readline())
                    if m['type']=='error': raise RuntimeError(m)
                    return m
                send(dict(type='hello', revision=4, min_revision=4, client=dict(kind='benchmark', platform='macos', name='session-list', version='1')))
                assert receive()['type']=='welcome'
                results=[]
                for i in range(2):
                    start=time.perf_counter()
                    if i==0:
                        send(dict(type='subscribe', id='list', operation=dict(kind='session.list', directory=str(REPO), headless=False, after=None, limit=None), select=None, resume=None))
                    else: send(dict(type='checkpoint', id='list', checkpoint=i))
                    while True:
                        m=receive()
                        if m['type']=='reading' and (i==0 or m['frame']['answers']==i): break
                    ms=(time.perf_counter()-start)*1000
                    assert m['frame']['unavailable'] is None
                    results.append((ms,m['frame']['body']))
                send(dict(type='cancel', id='list'))
                (ROOT/(label+'-initial.json')).write_text(json.dumps(results[0][1]))
                return results[1][0],results[1][1],dict(ms=results[0][0],**validate('served',results[0][1]))
        finally:
            proc.terminate()
            try: proc.wait(timeout=10)
            except subprocess.TimeoutExpired: proc.kill(); proc.wait()
            path.unlink(missing_ok=True)


def run(side, workload, label, trace=False):
    activate(side,trace)
    env=environment()
    if trace: env['SESSION_LIST_TRACE']=str(ROOT/(label+'-trace.jsonl'))
    row=dict(side=side, workload=workload, label=label, errors=0, load_average=os.getloadavg())
    try:
        if workload != 'served' and not trace:
            warm_ms, warm_data, _ = cli(workload, env)
            row['warmup'] = dict(ms=warm_ms, **validate(workload, warm_data))
        ms,data,extra = served(env,label) if workload=='served' else cli(workload,env)
        (ROOT/(label+'.json')).write_text(json.dumps(data))
        row.update(ms=ms, **validate(workload,data))
        row['warmup' if workload=='served' else 'stderr']=extra
    except Exception as e: row.update(errors=1,error=str(e))
    print(json.dumps(row),flush=True)
    with (ROOT/'trials.jsonl').open('a') as out: out.write(json.dumps(row)+'\n')
    return row


if __name__=='__main__':
    import sys
    run(*sys.argv[1:4])
