"""Time a real server subscription's requested snapshots; always stop the server."""
import hashlib,json,os,socket,subprocess,time
from run import ROOT,REPO,environment,check

def serve(label='served', count=5, binary=None, trace=None):
    env=environment(ROOT/'home')
    if trace: env['SESSION_LIST_TRACE']=str(trace)
    log=(ROOT/f'{label}.log').open('w')
    proc=subprocess.Popen([str(binary or ROOT/'lf'),'server','run'],env=env,cwd=REPO,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
    path=__import__('pathlib').Path('/tmp')/f'lf-{os.getuid()}-{hashlib.sha256(str((ROOT/"home").resolve()).encode()).hexdigest()[:16]}'/'machine.sock'
    try:
        end=time.monotonic()+30
        while not path.exists():
            if proc.poll() is not None or time.monotonic()>end: raise RuntimeError('server startup failed')
            time.sleep(.05)
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(120);sock.connect(str(path)); f=sock.makefile('rwb')
            def send(m): f.write(json.dumps(m).encode()+b'\n');f.flush()
            def receive():
                m=json.loads(f.readline())
                if m['type']=='error': raise RuntimeError(m)
                return m
            send(dict(type='hello',revision=4,min_revision=4,client=dict(kind='benchmark',platform='macos',name='session-list',version='1')))
            assert receive()['type']=='welcome'
            results=[]
            for i in range(count+1):
                t=time.perf_counter()
                if i==0: send(dict(type='subscribe',id='list',operation=dict(kind='session.list',directory=str(REPO),headless=False,after=None,limit=None),select=None,resume=None))
                else: send(dict(type='checkpoint',id='list',checkpoint=i))
                while True:
                    m=receive()
                    if m['type']=='reading' and (i==0 or m['frame']['answers']==i): break
                ms=(time.perf_counter()-t)*1000
                frame=m['frame']; assert frame['unavailable'] is None,frame['unavailable']
                row=dict(label=label,run=i,ms=ms,errors=0,**check(frame['body'],ROOT/'home/loopflow.db'))
                (ROOT/f'{label}-{i}.json').write_text(json.dumps(frame['body']))
                results.append(row);print(json.dumps(row),flush=True)
                (ROOT/f'{label}-results.json').write_text(json.dumps(results,indent=2))
            send(dict(type='cancel',id='list'))
    finally:
        proc.terminate()
        try: proc.wait(timeout=10)
        except subprocess.TimeoutExpired: proc.kill();proc.wait()
        log.close()
        path.unlink(missing_ok=True)

if __name__=='__main__':
    import sys
    serve(sys.argv[1] if len(sys.argv)>1 else 'served')
