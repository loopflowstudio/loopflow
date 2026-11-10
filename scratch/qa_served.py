import json
import os
from pathlib import Path
import select
import socket
import sqlite3
import subprocess
import tempfile
import time
import traceback

ROOT = Path.cwd()
BIN = ROOT / 'target/debug/lf'
INSTALLED = Path('/Applications/Loopflow.app/Contents/MacOS/lf')
BASE = Path(tempfile.mkdtemp(prefix='loo790-', dir='/tmp'))
REPO = BASE / 'repo'
ENV = {k: v for k, v in os.environ.items() if not k.startswith('LF_')}
ENV.update(HOME=str(BASE / 'home'), LF_HOME=str(BASE / 'state'), LF_BIN=str(BIN))
for p in [REPO, BASE / 'home', BASE / 'state']:
    p.mkdir()
EVENTS = []
CHILDREN = []

def record(name, value):
    EVENTS.append({'scenario': name, 'result': value})
    (ROOT / 'scratch/qa-transcript.json').write_text(json.dumps(EVENTS, indent=2))
    print(name + ': ' + json.dumps(value)[:1500], flush=True)

def run(args, cwd=REPO, binary=None):
    command = [str(binary or BIN), *args] if args[0] != 'git' else args
    result = subprocess.run(command, cwd=cwd, env=ENV, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stderr} {result.stdout}')
    return result.stdout

def cli(*args, binary=None):
    return json.loads(run(list(args), binary=binary))

def revisions():
    with sqlite3.connect(f'file:{BASE}/state/loopflow.db?mode=ro', uri=True) as db:
        return db.execute('SELECT domain, revision FROM store_revisions ORDER BY domain').fetchall()

def start_server():
    log = (BASE / 'server.log').open('a')
    p = subprocess.Popen([str(BIN), 'server', 'run'], cwd=REPO, env=ENV, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
    CHILDREN.append(p)
    for _ in range(100):
        status = cli('server', 'status')
        if status.get('ready'):
            record('server ready', status)
            return p, status['socket']
        if p.poll() is not None:
            raise RuntimeError((BASE / 'server.log').read_text())
        time.sleep(.1)
    raise RuntimeError(status)

class Client:
    def __init__(self, path, hello=None):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.connect(path)
        self.buf = b''
        self.send(hello or json.loads((ROOT / 'tests/fixtures/dto/server/hello.json').read_text()))
        self.welcome = self.recv()
        assert self.welcome['type'] == 'welcome', self.welcome
    def send(self, msg):
        self.sock.sendall(json.dumps(msg).encode() + b'\n')
    def recv(self, timeout=15):
        deadline = time.monotonic() + timeout
        while b'\n' not in self.buf:
            if not select.select([self.sock], [], [], max(0, deadline-time.monotonic()))[0]:
                raise TimeoutError('No frame')
            chunk = self.sock.recv(1024*1024)
            if not chunk:
                raise EOFError()
            self.buf += chunk
        line, self.buf = self.buf.split(b'\n', 1)
        return json.loads(line)
    def request(self, op, id='request'):
        self.send(dict(type='request', id=id, operation=op, select=None))
        result = self.recv()
        assert result['type'] == 'response', result
        return result['result']
    def subscribe(self, op, id='watch', selection=None, resume=None):
        self.send(dict(type='subscribe', id=id, operation=op, select=selection, resume=resume))
        response = self.recv()
        assert response['type'] == 'response', response
        return self.recv()
    def until(self, predicate):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            value = self.recv(deadline-time.monotonic())
            if predicate(value):
                return value
        raise TimeoutError()
    def close(self):
        self.sock.close()

def dirty(frame):
    return frame['frame']['body']['row']['condition']['local_progress']['dirty']

def main():
    record('isolation', str(BASE))
    run(['git', 'init', '-b', 'main'])
    run(['git', 'config', 'user.name', 'QA'])
    run(['git', 'config', 'user.email', 'qa@example.test'])
    run(['git', 'init', '--bare', str(BASE / 'origin.git')])
    run(['git', 'remote', 'add', 'origin', str(BASE / 'origin.git')])
    (REPO / '.lf/graphs').mkdir(parents=True)
    (REPO / '.lf/config.yaml').write_text('default_agent: codex\nplanning: {provider: git, personal: true}\ngraph: qa-normal\n')
    (REPO / '.lf/graphs/qa-normal.yaml').write_text('- cmd: help\n')
    (REPO / 'edit.txt').write_text('before\n')
    run(['git', 'add', '.'])
    run(['git', 'commit', '-m', 'QA baseline'])
    run(['git', 'push', '-u', 'origin', 'main'])
    old = cli('session', 'resume', '--prepare', '--json', binary=INSTALLED)
    record('installed Session prepared', old)
    now = cli('session', 'resume', '--prepare', '--json')
    record('branch Session prepared', now)
    assert old['id'] == now['id']
    worktree = cli('wt', 'create', 'observations', '--json')
    record('checkout', worktree)
    wt = Path(worktree['path'])
    task = cli('task', 'create', '--title', 'Observed edits QA', '--worktree', str(wt), '--json')
    record('created Task', task)
    issue = task['id']
    server, path = start_server()
    c = Client(path)
    pane = dict(kind='task.get', directory=str(REPO), issue=issue)
    before = c.subscribe(pane)
    record('clean baseline', before)
    assert dirty(before) is False
    selected = Client(path)
    baseline = selected.subscribe(pane, selection=['task.title'])
    record('selected baseline', baseline)
    rev = revisions()
    (wt / 'edit.txt').write_text('after\n')
    (wt / 'new.txt').write_text('new\n')
    started = time.monotonic()
    changed = c.until(lambda m: m['type'] == 'reading' and dirty(m) is True)
    assert revisions() == rev, (rev, revisions())
    record('external edit wakes without store writes', {'seconds': round(time.monotonic()-started, 2), 'frame': changed})
    try:
        unexpected = selected.recv(4)
        raise AssertionError(unexpected)
    except TimeoutError:
        record('unchanged selected title', 'No frame after unrelated file change')
    query = Client(path)
    changes = dict(kind='task.changes', issue=issue, base='parent', head=None)
    record('served changed files', query.request(changes))
    record('bare CLI changed files', cli('task', 'changes', issue, '--json'))
    record('Session upgrade read', query.request(dict(kind='session.list', directory=str(REPO), headless=True, after=None, limit=None)))
    # A missing checkout becomes visible again without a registry write.
    moved = wt.with_name(wt.name + '.away')
    wt.rename(moved)
    try:
        missing = c.until(lambda m: m['type'] == 'reading' and dirty(m) is None)
        record('missing checkout', missing)
    finally:
        moved.rename(wt)
    restored = c.until(lambda m: m['type'] == 'reading' and dirty(m) is True)
    record('checkout restored', restored)
    assert revisions() == rev
    # Exercise the actual Swift encoder against the running Rust server.
    messages = [json.loads(line) for line in subprocess.check_output(['/tmp/loo790-qa-encode', str(REPO), issue], text=True).splitlines()]
    swift = Client(path, hello=messages[0])
    swift.send(messages[1])
    swift_answer = swift.recv()
    assert swift_answer['result']['task']['title'] == 'Observed edits QA'
    swift.send(messages[2])
    assert swift.recv()['type'] == 'response'
    swift_baseline = swift.recv()
    assert dirty(swift_baseline) is True
    record('Swift encoded live requests and subscription', [swift_answer, swift_baseline])
    swift.close()
    # File reads, large content, and Git argument changes reach the actual checkout.
    file_op = dict(kind='task.file', issue=issue, path='edit.txt')
    record('file initial content', query.request(file_op))
    (wt / 'edit.txt').write_text('large line\n' * 60000)
    end = time.monotonic()+10
    while True:
        content = query.request(file_op)
        if 'large line' in json.dumps(content):
            break
        assert time.monotonic()<end
        time.sleep(.2)
    record('large file observed', {k: (str(v)[:120] if k in ['content', 'text'] else v) for k, v in content.items()})
    record('alternate Git comparison', query.request({**changes, 'base': 'head'}))
    query.send(dict(type='request', id='bad-base', operation={**changes, 'base': 'HEAD'}, select=None))
    invalid = query.recv()
    assert invalid['type'] == 'error', invalid
    record('invalid comparison returns actionable error', invalid)
    assert query.request(pane)['task']['title'] == 'Observed edits QA'
    # Discovery follows external file creation and content changes.
    graphs = dict(kind='graph.list', directory=str(REPO))
    initial_graphs = query.request(graphs)
    (REPO / '.lf/graphs/qa-added.yaml').write_text('- cmd: help\n')
    end = time.monotonic()+10
    while True:
        observed = query.request(graphs)
        if 'qa-added' in json.dumps(observed):
            break
        assert time.monotonic()<end
        time.sleep(.2)
    record('external Graph creation', {'before': len(initial_graphs), 'after': len(observed), 'added': [g for g in observed if 'qa-added' in json.dumps(g)]})
    config = dict(kind='config.settings', directory=str(REPO), wave=None, skill=None, graph=None, step=None)
    initial_config = query.request(config)
    config_file = REPO / '.lf/config.yaml'
    config_file.write_text(config_file.read_text().replace('default_agent: codex', 'default_agent: claude'))
    end = time.monotonic()+10
    while True:
        observed_config = query.request(config)
        if observed_config != initial_config:
            break
        assert time.monotonic()<end
        time.sleep(.2)
    record('external configuration edit', [{'agent': item['settings']['agent']} for item in observed_config])
    good_config = config_file.read_text()
    config_file.write_text('default_agent: [\n')
    end = time.monotonic()+10
    while True:
        query.send(dict(type='request', id='broken-config', operation=config, select=None))
        failed = query.recv()
        if failed['type'] == 'error':
            break
        assert time.monotonic()<end
        time.sleep(.2)
    record('configuration failure surfaced', failed)
    config_file.write_text(good_config)
    end = time.monotonic()+10
    while True:
        query.send(dict(type='request', id='recovered-config', operation=config, select=None))
        recovered_config = query.recv()
        if recovered_config['type'] == 'response':
            break
        assert time.monotonic()<end
        time.sleep(.2)
    record('configuration failure recovered', recovered_config['result'][0]['settings']['agent'])
    # Reconnect with the title checkpoint: no duplicate body; explicit checkpoint answers.
    selected.close()
    selected = Client(path)
    selected.send(dict(type='subscribe', id='resume', operation=pane, select=['task.title'], resume=baseline['checkpoint']))
    assert selected.recv()['type'] == 'response'
    try:
        raise AssertionError(selected.recv(2))
    except TimeoutError:
        pass
    selected.send(dict(type='checkpoint', id='resume', checkpoint=42))
    checked = selected.recv()
    assert checked['frame']['answers'] == 42
    record('reconnect unchanged selection', checked)
    query.close()
    c.close()
    selected.close()
    server.terminate()
    server.wait(timeout=10)
    # Edits during an outage must form the replacement server's baseline.
    (wt / 'edit.txt').write_text('before\n')
    (wt / 'new.txt').unlink()
    replacement, new_path = start_server()
    recovered = Client(new_path)
    assert recovered.welcome['instance'] != c.welcome['instance']
    fresh = recovered.subscribe(pane)
    assert dirty(fresh) is False
    record('server replacement observes offline edit', {'instance': recovered.welcome['instance'], 'dirty': dirty(fresh)})
    (wt / 'new-again.txt').write_text('post-restart\n')
    again = recovered.until(lambda m: m['type'] == 'reading' and dirty(m) is True)
    record('replacement continues observing', {'dirty': dirty(again)})
    recovered.close()
    # A real CLI invocation waiting for its Task description, killed without cleanup.
    pending = subprocess.Popen([str(BIN), 'task', 'create', '--title', 'Unsubmitted input'], cwd=REPO, env=ENV, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    CHILDREN.append(pending)
    time.sleep(.5)
    assert pending.poll() is None
    processes = Client(new_path)
    live = processes.subscribe(dict(kind='process.list'))
    if not any(node['pid'] == pending.pid for node in live['frame']['body']['nodes']):
        live = processes.until(lambda m: m['type'] == 'reading' and any(node['pid'] == pending.pid for node in m['frame']['body']['nodes']))
    record('live CLI invocation in activity', live)
    process_rev = revisions()
    pending.kill()
    pending.wait(timeout=10)
    dead = processes.until(lambda m: m['type'] == 'reading' and all(node['pid'] != pending.pid for node in m['frame']['body']['nodes']))
    assert revisions() == process_rev
    record('abrupt process exit wakes without store writes', dead)
    processes.close()
    replacement.terminate()
    replacement.wait(timeout=10)
    record('bare CLI with server stopped', cli('task', 'changes', issue, '--json'))

try:
    main()
except Exception:
    record('FAILURE', traceback.format_exc())
    raise
finally:
    for child in CHILDREN:
        if child.poll() is None:
            child.kill()
        child.wait(timeout=10)
    record('cleanup', 'Every server child stopped and reaped')
