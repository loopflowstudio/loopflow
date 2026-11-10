import json
import os
from pathlib import Path
import select
import sqlite3
import subprocess
import time

root = Path.cwd()
events = json.loads((root / 'scratch/qa-transcript.json').read_text())
base = Path(next(e['result'] for e in events if e['scenario'] == 'isolation'))
env = {k: v for k, v in os.environ.items() if not k.startswith('LF_')}
env.update(HOME=str(base / 'home'), LF_HOME=str(base / 'state'), LF_BIN=str(root / 'target/debug/lf'))
children = []
def spawn(args, **kw):
    child = subprocess.Popen([env['LF_BIN'], *args], cwd=base / 'repo', env=env, stdin=subprocess.PIPE, stderr=subprocess.PIPE, **kw)
    children.append(child)
    return child

def revisions():
    with sqlite3.connect(f'file:{base}/state/loopflow.db?mode=ro', uri=True) as db:
        return db.execute('SELECT domain, revision FROM store_revisions ORDER BY domain').fetchall()

buf = b''
def frame(child):
    global buf
    deadline = time.monotonic() + 15
    while b'\n' not in buf:
        assert select.select([child.stdout], [], [], max(0, deadline-time.monotonic()))[0], 'no CLI frame'
        chunk = os.read(child.stdout.fileno(), 1024*1024)
        assert chunk, child.stderr.read().decode()
        buf += chunk
    line, buf = buf.split(b'\n', 1)
    return json.loads(line)

try:
    pending = spawn(['task', 'create', '--title', 'Interrupted CLI input'], stdout=subprocess.DEVNULL)
    time.sleep(.5)
    watch = spawn(['process', 'list', '--subscribe', '--json'], stdout=subprocess.PIPE)
    before = frame(watch)
    assert any(n['pid'] == pending.pid for n in before['body']['nodes']), before
    rev = revisions()
    pending.kill()
    pending.wait(timeout=10)
    after = frame(watch)
    assert all(n['pid'] != pending.pid for n in after['body']['nodes']), after
    assert revisions() == rev
    event = {'scenario': 'CLI subscription without server', 'result': {'before': before, 'after': after, 'store_revisions_unchanged': True}}
    events.append(event)
    print(json.dumps(event, indent=2))
finally:
    for child in children:
        if child.poll() is None:
            child.kill()
        child.wait(timeout=10)
    events.append({'scenario': 'CLI cleanup', 'result': 'Both CLI children stopped and reaped'})
    (root / 'scratch/qa-transcript.json').write_text(json.dumps(events, indent=2))
