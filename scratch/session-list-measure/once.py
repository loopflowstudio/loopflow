"""Trace the server's one-shot read separately from its subscription path."""
import hashlib,json,os,socket,subprocess,time
from run import ROOT,REPO,environment,check

env=environment(ROOT/'home');env['SESSION_LIST_TRACE']=str(ROOT/'once-trace.jsonl')
path=__import__('pathlib').Path('/tmp')/f'lf-{os.getuid()}-{hashlib.sha256(str((ROOT/"home").resolve()).encode()).hexdigest()[:16]}'/'machine.sock'
with (ROOT/'once.log').open('w') as log:
    proc=subprocess.Popen([str(ROOT/'lf-trace'),'server','run'],env=env,cwd=REPO,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
    try:
        until=time.monotonic()+30
        while not path.exists():
            if proc.poll() is not None or time.monotonic()>until:raise RuntimeError('server startup failed')
            time.sleep(.05)
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(120);s.connect(str(path));f=s.makefile('rwb')
            def send(m):f.write(json.dumps(m).encode()+b'\n');f.flush()
            send(dict(type='hello',revision=4,min_revision=4,client=dict(kind='benchmark',platform='macos',name='session-list',version='1')))
            assert json.loads(f.readline())['type']=='welcome'
            send(dict(type='request',id='once',operation=dict(kind='session.list',directory=str(REPO),headless=False,after=None,limit=None),select=None))
            m=json.loads(f.readline());assert m['type']=='response',m
            print(json.dumps(check(m['result'],ROOT/'home/loopflow.db')))
    finally:
        proc.terminate()
        try:proc.wait(timeout=10)
        except subprocess.TimeoutExpired:proc.kill();proc.wait()
        path.unlink(missing_ok=True)
