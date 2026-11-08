import hashlib
import json
import os
from pathlib import Path
import signal
import select
import socket
import subprocess
import tempfile
import time

BINARY = Path('target/debug/lf').resolve()
ROOT = Path(tempfile.mkdtemp(prefix='loo804-qa-', dir='/tmp')).resolve()
HOME = ROOT / 'home'
LFHOME = ROOT / 'machine'
REPO = ROOT / 'repo'
for path in (HOME, LFHOME, REPO / '.lf/graphs'):
    path.mkdir(parents=True)
ENV = {'PATH': os.environ['PATH'], 'HOME': str(HOME), 'LF_HOME': str(LFHOME), 'LF_BIN': str(BINARY), 'TERM': 'dumb'}
TRANSCRIPT = []
STOPPED = set()

def record(label, value):
    event = {'scenario': label, 'value': value}
    TRANSCRIPT.append(event)
    print(json.dumps(event), flush=True)
    Path('scratch/qa-task-transcript.json').write_text(json.dumps(TRANSCRIPT, indent=2))

def cli(*args):
    result = subprocess.run([str(BINARY), *args], cwd=REPO, env=ENV, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=30)
    return {'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}

for args in [('init', '-q'), ('config', 'user.name', 'Launch QA'), ('config', 'user.email', 'qa@example.invalid'), ('commit', '--allow-empty', '-qm', 'Initialize launch QA')]:
    subprocess.run(['git', *args], cwd=REPO, env=ENV, check=True, capture_output=True)
for name in ('normal', 'killed', 'unknown', 'recovered', 'live'):
    (REPO / f'.lf/graphs/qa-{name}.yaml').write_text('- cmd: help\n')

(REPO / '.lf/config.yaml').write_text('planning: {provider: git, personal: true}\ngraph: qa-normal\n')
subprocess.run(['git', 'branch', '-M', 'main'], cwd=REPO, env=ENV, check=True)
subprocess.run(['git', 'init', '--bare', '-q', str(ROOT / 'origin.git')], env=ENV, check=True)
subprocess.run(['git', 'remote', 'add', 'origin', str(ROOT / 'origin.git')], cwd=REPO, env=ENV, check=True)
subprocess.run(['git', 'add', '.lf'], cwd=REPO, env=ENV, check=True)
subprocess.run(['git', 'commit', '-qm', 'Add command-only QA graphs'], cwd=REPO, env=ENV, check=True)
subprocess.run(['git', 'push', '-qu', 'origin', 'main'], cwd=REPO, env=ENV, check=True)
SOCKET = Path('/tmp').resolve() / f'lf-{os.geteuid()}-{hashlib.sha256(str(LFHOME).encode()).hexdigest()[:16]}' / 'machine.sock'
LOG = open(ROOT / 'server.log', 'w')
SERVER = subprocess.Popen([str(BINARY), 'server', 'run'], cwd=REPO, env=ENV, stdin=subprocess.DEVNULL, stdout=LOG, stderr=LOG, start_new_session=True)

def connection():
    sock = socket.socket(socket.AF_UNIX)
    sock.settimeout(20)
    sock.connect(str(SOCKET))
    file = sock.makefile('rwb', buffering=0)
    file.write((json.dumps({'type': 'hello', 'revision': 2, 'min_revision': 2, 'client': {'kind': 'cli', 'platform': 'macos', 'name': 'LOO-804 QA', 'version': 'branch'}}) + '\n').encode())
    welcome = json.loads(file.readline())
    assert welcome['type'] == 'welcome', welcome
    return sock, file

def send(file, identifier, operation):
    file.write((json.dumps({'type': 'request', 'id': identifier, 'operation': operation, 'select': None}) + '\n').encode())

def request(identifier, operation):
    sock, file = connection()
    try:
        send(file, identifier, operation)
        return json.loads(file.readline())
    finally:
        file.close()
        sock.close()

def graph(name):
    return {'kind': 'graph.run', 'directory': str(REPO), 'name': 'qa-' + name, 'message': None, 'agent': None}

def process_rows():
    rows = []
    output = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,command='], text=True)
    for line in output.splitlines():
        fields = line.strip().split(None, 2)
        if len(fields) == 3:
            rows.append((int(fields[0]), int(fields[1]), fields[2]))
    return rows

def suspend_launch(name, operation):
    sock, file = connection()
    send(file, name, operation)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        for pid, parent, command in process_rows():
            if command.startswith(str(BINARY) + ' task run ' + name):
                os.kill(pid, signal.SIGSTOP)
                STOPPED.add(pid)
                time.sleep(.2)
                record(name + ' suspended', {'child_pid': pid, 'owner_pid': parent, 'reports': [p.name for p in SOCKET.parent.glob('report-*.json')]})
                return sock, file, pid, parent
        time.sleep(.002)
    raise RuntimeError('could not intercept launch ' + name)

def resume(pid):
    os.kill(pid, signal.SIGCONT)
    STOPPED.discard(pid)

def hide_registry():
    saved = ROOT / 'hidden'
    saved.mkdir(exist_ok=True)
    for name in ('loopflow.db', 'loopflow.db-wal', 'loopflow.db-shm'):
        path = LFHOME / name
        if path.exists():
            path.rename(saved / name)
    (LFHOME / 'loopflow.db').mkdir()

def restore_registry():
    (LFHOME / 'loopflow.db').rmdir()
    for path in (ROOT / 'hidden').iterdir():
        path.rename(LFHOME / path.name)


try:
    deadline = time.monotonic() + 15
    while not SOCKET.exists():
        if SERVER.poll() is not None or time.monotonic() > deadline:
            raise RuntimeError((ROOT / 'server.log').read_text())
        time.sleep(.05)
    record('environment', {'root': str(ROOT), 'server_pid': SERVER.pid})
    for scenario in ('normal Task', 'late Task'):
        created = cli('task', 'create', '--title', scenario, '--json')
        record('create ' + scenario, created)
        assert created['exit'] == 0
        task = json.loads(created['stdout'])['identifier']
        operation = {'kind': 'task.run', 'directory': str(REPO), 'issue': task, 'from': None, 'agent': None, 'approve': False}
        if scenario == 'normal Task':
            record('served Task launch', request(task, operation))
        else:
            sock, file, child, owner = suspend_launch(task, operation)
            record('independent read while launch waits', request('parallel-read', {'kind': 'graph.list', 'directory': str(REPO)}))
            os.kill(owner, signal.SIGSTOP); STOPPED.add(owner)
            hide_registry()
            start = time.monotonic()
            answer = json.loads(file.readline())
            record('Task launch uncertain', {'seconds': round(time.monotonic() - start, 2), 'reply': answer, 'child_alive': os.kill(child, 0) is None})
            assert answer['code'] == 'outcome_unknown'
            restore_registry()
            resume(owner); resume(child)
            file.close(); sock.close()
        deadline = time.monotonic() + 20
        while True:
            runs = cli('task', 'runs', task, '--json')
            if '"completed"' in runs['stdout'] or time.monotonic() > deadline:
                break
            time.sleep(.2)
        record(scenario + ' runs', runs)
        record(scenario + ' status', cli('task', 'status', task, '--json'))
    record('finished Graph inventory', cli('graph', 'list', '--processes', '--json'))
    record('final server status', cli('server', 'status'))

finally:
    if (ROOT / 'hidden').exists() and (LFHOME / 'loopflow.db').is_dir():
        restore_registry()
    for pid in list(STOPPED):
        try: resume(pid)
        except ProcessLookupError: pass
    rows = process_rows()
    descendants = {SERVER.pid}
    for _ in range(20):
        before = len(descendants)
        descendants.update(pid for pid, parent, _ in rows if parent in descendants)
        if len(descendants) == before: break
    for pid in sorted(descendants, reverse=True):
        try: os.kill(pid, signal.SIGTERM)
        except ProcessLookupError: pass
    try: SERVER.wait(timeout=5)
    except subprocess.TimeoutExpired:
        SERVER.kill(); SERVER.wait()
    LOG.close()
    record('cleanup', {'server_exit': SERVER.returncode, 'remaining_owned_pids': [pid for pid, _, _ in process_rows() if pid in descendants]})
