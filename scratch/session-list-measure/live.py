"""Alternate identical-store CLI reads with and without a real Pi writer."""
import json,sqlite3,subprocess,time,shutil
from run import ROOT,REPO,environment,run

control=ROOT/'control';control.mkdir(exist_ok=True)
with sqlite3.connect(ROOT/'home/loopflow.db') as src,sqlite3.connect(control/'loopflow.db') as dst: src.backup(dst)
# Unchanged store snapshot, same binary, flags, checkout and provider homes.
work=ROOT/'provider-work';work.mkdir(exist_ok=True)
subprocess.run(['git','init','-q',str(work)],check=True)
prompt=f'''Read these Loopflow source files and explain where session listing could wait behind a writer or scan excessive data: {REPO}/rust/loopflow/src/ops/human_session/turns.rs, {REPO}/rust/loopflow/src/store/sqlite/sessions.rs, {REPO}/rust/loopflow/src/store/sqlite/session_events.rs, {REPO}/rust/loopflow/src/observe.rs. Give concrete query and function names and distinguish SQLite waits from filesystem or process discovery. This is a read-only investigation: do not edit files, run builds, invoke lf, or access credentials. Return your findings in your final response.'''
env=environment(ROOT/'home')
began=int(time.time())
log=(ROOT/'provider.log').open('w')
writer=subprocess.Popen([str(ROOT/'lf'),'--cwd',str(work),'-b','-a','pi',':',prompt],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
receipts=[]
def events():
    with sqlite3.connect(f'file:{ROOT}/home/loopflow.db?mode=ro',uri=True) as db:
        return db.execute('SELECT count(*),max(seq) FROM session_events').fetchone()
try:
    first=events(); deadline=time.monotonic()+90
    while events()[0]<first[0]+4:
        if writer.poll() is not None or time.monotonic()>deadline: raise RuntimeError('provider did not start writing')
        time.sleep(.1)
    for i in range(5):
        before=events();alive=writer.poll() is None
        order=['control','live'] if i%2==0 else ['live','control']
        for side in order:
            run(f'{side}-{i}',1,control if side=='control' else ROOT/'home',[])
        after=events()
        receipts.append(dict(pair=i,before=before,after=after,writer_alive_before=alive,writer_alive_after=writer.poll() is None))
        (ROOT/'writer-receipts.json').write_text(json.dumps(receipts,indent=2))
        print(json.dumps(receipts[-1]),flush=True)
        if writer.poll() is not None: break
        # Natural provider writes, no synthetic lock holder or load generator.
        until=time.monotonic()+10
        while events()==after and writer.poll() is None and time.monotonic()<until: time.sleep(.1)
    try: code=writer.wait(timeout=120)
    except subprocess.TimeoutExpired: code=None
    print(json.dumps(dict(writer_exit=code,initial=first,final=events())),flush=True)
finally:
    # A headless run's transport detaches; terminating only the CLI does not
    # stop its provider. Select only the Session this invocation just created.
    import os,signal
    with sqlite3.connect(ROOT/'home/loopflow.db') as db:
        pids=[r[0] for r in db.execute(
            'SELECT p.pid FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.cwd=? AND s.created_at>=?',
            (str(work.resolve()),began)) if r[0]]
    for pid in pids:
        try: os.kill(pid,signal.SIGTERM)
        except ProcessLookupError: pass
    if writer.poll() is None:
        import os,signal
        os.killpg(writer.pid,signal.SIGTERM)
        try:writer.wait(timeout=10)
        except subprocess.TimeoutExpired:os.killpg(writer.pid,signal.SIGKILL);writer.wait()
    log.close()
