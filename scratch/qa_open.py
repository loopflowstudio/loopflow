from pathlib import Path
import os, subprocess, json, tempfile, time, signal, sqlite3, html, shutil

CHECKOUT = Path.cwd()
ROOT = Path(tempfile.mkdtemp(prefix='lf-qa-open-', dir='/tmp'))
REPO = ROOT/'repo'
LF = str(CHECKOUT/'target/debug/lf')
REX = shutil.which('rex')
ENV = {k:v for k,v in os.environ.items() if not k.startswith(('LF_', 'REX_', 'HERDR_', 'CMUX_', 'TMUX', 'CODEX_', 'CLAUDE_')) and k not in ('HOME','XDG_CONFIG_HOME','XDG_STATE_HOME')}
ENV.update(HOME=str(ROOT/'home'), LF_HOME=str(ROOT/'machine'), LF_BIN=LF, CODEX_HOME=str(ROOT/'home/codex'), CLAUDE_CONFIG_DIR=str(ROOT/'home/claude'), XDG_CONFIG_HOME=str(ROOT/'config'), XDG_STATE_HOME=str(ROOT/'state'), TERM='xterm-256color')
for path in [REPO, ROOT/'home/codex', ROOT/'machine', ROOT/'bin']:
    path.mkdir(parents=True)
(ROOT/'home/codex/config.toml').write_text('cli_auth_credentials_store = "file"\n')
(ROOT/'empty.lua').write_text('')
records=[]
children=[]

def run(args, env=None, timeout=45, cwd=REPO, record=True):
    actual=ENV.copy(); actual.update(env or {})
    p=subprocess.run([str(x) for x in args],cwd=cwd,env=actual,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=timeout)
    if record:
        row={'command':' '.join(str(x) for x in args).replace(LF,'lf'),'code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
        records.append(row)
        print(json.dumps(row),flush=True)
        (CHECKOUT/'scratch/qa-open-transcript.json').write_text(json.dumps(records,indent=2))
    return p

def cli(*args, **kwargs): return run([LF,*args],**kwargs)
def data(*args,**kwargs):
    p=cli(*args,**kwargs)
    if p.returncode: raise RuntimeError(p.stderr or p.stdout)
    return json.loads(p.stdout)

def start(args, name, env=None):
    actual=ENV.copy(); actual.update(env or {})
    log=open(ROOT/(name+'.log'),'w')
    p=subprocess.Popen(args,cwd=REPO,env=actual,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
    children.append(p)
    return p

def rex(method,args):
    p=run([REX,'--autostart=false','--timeout','5s','-S',endpoint,'api','call',method,json.dumps(args)])
    if p.returncode: raise RuntimeError(p.stderr)
    return json.loads(p.stdout)

def capture(session):
    blocks=rex('session.list_blocks',{'session_id':session})['blocks']
    for block in blocks:
        run([REX,'--autostart=false','-S',endpoint,'block','capture','-s',session,'-b',block['block_id']])
    return blocks

try:
    run(['git','init','-b','main'])
    run(['git','config','user.name','Opening QA'])
    run(['git','config','user.email','qa@example.invalid'])
    run(['git','init','--bare',str(ROOT/'origin.git')])
    run(['git','remote','add','origin',str(ROOT/'origin.git')])
    (REPO/'.lf').mkdir()
    (REPO/'.lf/config.yaml').write_text('default_agent: codex\nplanning: {provider: git, personal: true}\n')
    (REPO/'README.md').write_text('# Opening QA\n')
    run(['git','add','.']); run(['git','commit','-m','Initialize opening QA']); run(['git','push','-u','origin','main'])
    task=data('task','create','--title','Recover a report','--request-id','qa-report','--json')['task']
    start([LF,'server','run'],'lf-server')
    for _ in range(50):
        status=data('server','status')
        if status.get('ready'):break
        time.sleep(.2)
    cli('help','open')
    cli('open','--focus','--json')
    cli('session','list','--json')
    cli('open','--tab','--host','terminal','--json')
    cli('open','--here','--tab','--json')
    prepared=data('resume','--prepare','--json')
    (ROOT/'prepared.json').write_text(json.dumps(prepared))
    endpoint='unix://'+str(ROOT/'rex.sock')
    start([REX,'server','run','--listen',endpoint,'--state-dir',str(ROOT/'rex-state'),'--config',str(ROOT/'empty.lua')],'rex')
    for _ in range(50):
        if (ROOT/'rex.sock').exists():break
        time.sleep(.1)
    result=data('open','--tab','--host','rex','--server',endpoint,'--json',timeout=50)
    receipt=result['receipts'][0]
    container=receipt['container']
    time.sleep(2)
    capture(container)
    cli('open','--tab','--host','rex','--server',endpoint,'--json')
    cli('open','--focus','--json')
    cli('session','list','--json')
    # Direct CLI opening inside a real Rex block must stay in that block.
    direct = rex('session.new_window', {'session_id':container, 'label':'Direct Task opening', 'layout':{'block':{'flavor':'com.superlogical.terminal.shell','label':'Direct CLI','options':{'cwd':str(REPO),'shell':'none','command':[LF,'open','--task',task['id']]}}}})
    time.sleep(4)
    blocks=capture(container)
    sessions=data('session','list','--json')
    task_session=next(row for row in sessions if task['id'] in row['task_ids'])
    cli('task','status',task['id'],'--json')
    # Reproduce the tool-call environment using a real private provider's provenance.
    with sqlite3.connect(f"file:{ROOT}/machine/loopflow.db?mode=ro",uri=True) as db:
        agent=db.execute("select id,parent_lf_process_id from processes where kind='agent' and agent_session_id=? order by started_at desc",(task_session['id'],)).fetchone()
    caller={'LF_AGENT_CALLER':json.dumps({'session_id':task_session['id'],'agent_process_id':agent[0],'origin_exec_id':agent[1]})}
    cli('open','--session',task_session['id'],'--json',env=caller)
    cli('open','--session',task_session['id'],'--here','--json',env=caller)
    # A real Wave opens a conversation in a sibling tab in the agent's Rex container.
    data('wave','create','reports','--objective','Recover reports reliably','--json')
    cli('open','--wave','reports','--focus','--json')
    wave_open=data('open','--wave','reports','--json',env=caller)
    time.sleep(2)
    capture(container)
    cli('open','--wave','reports','--json',env=caller)
    cli('session','list','--json')
    # Interrupt the original provider at the sign-in screen, then reopen saved history.
    original=next(block for block in blocks if block['window_id']==receipt['destination'])
    run([REX,'--autostart=false','-S',endpoint,'block','call','-s',container,'-b',original['block_id'],'write',json.dumps({'data':'\x03'})])
    time.sleep(3)
    cli('session','card',prepared['id'],'--json')
    # Bad real provider configuration is a startup failure, not a mocked host reply.
    (ROOT/'home/codex/config.toml').write_text('this is not valid TOML [\n')
    failed=cli('open','--session',prepared['id'],'--tab','--host','rex','--server',endpoint,'--json',timeout=50)
    if failed.stdout.strip():
        failure=json.loads(failed.stdout)['receipts'][0]
        if failure['container']:capture(failure['container'])
    (ROOT/'home/codex/config.toml').write_text('cli_auth_credentials_store = "file"\n')
    recovered=cli('open','--session',prepared['id'],'--tab','--host','rex','--server',endpoint,'--json',timeout=50)
    if recovered.stdout.strip():
        recovery=json.loads(recovered.stdout)['receipts'][0]
        if recovery['container']:
            time.sleep(3)
            capture(recovery['container'])
    cli('session','card',prepared['id'],'--json')
    # Explicit workspace overview starts no extra conversations.
    before=data('session','list','--json')
    cli('workspace','--wave','reports','--host','rex','--server',endpoint,'--json')
    after=data('session','list','--json')
    records.append({'check':'workspace overview Session identities unchanged','passed':{x['id'] for x in before}=={x['id'] for x in after}})
    # Herdr's real headless server can also confirm tab selection.
    herdr=shutil.which('herdr')
    start([herdr,'--session','qa','server'],'herdr')
    herdr_socket=ROOT/'config/herdr/sessions/qa/herdr.sock'
    for _ in range(50):
        if herdr_socket.exists():break
        time.sleep(.1)
    henv={'HERDR_SOCKET_PATH':str(herdr_socket)}
    parallel_task=data('task','create','--title','Review concurrent opening','--json')['task']
    secondary=data('resume','--task',parallel_task['id'],'--prepare','--json')
    from concurrent.futures import ThreadPoolExecutor
    def concurrent_open(_):
        return cli('open','--session',secondary['id'],'--tab','--host','herdr','--json',env=henv)
    with ThreadPoolExecutor(max_workers=2) as pool:
        replies=list(pool.map(concurrent_open,range(2)))
    records.append({'check':'concurrent Herdr opens','statuses':[json.loads(p.stdout)['receipts'][0]['status'] for p in replies]})
    cli('open','--session',secondary['id'],'--focus','--json',env=henv)
    panes=run([herdr,'pane','list'],env=henv)
    for pane in json.loads(panes.stdout)['result']['panes']:
        run([herdr,'pane','read',pane['pane_id'],'--source','visible','--format','text'],env=henv)
    # Upgrade a Session prepared by the installed release, then read with this branch.
    installed='/Applications/Loopflow.app/Contents/MacOS/lf'
    upgrade={'LF_HOME':str(ROOT/'upgrade-machine'),'LF_BIN':installed}
    (ROOT/'upgrade-machine').mkdir()
    old=run([installed,'resume','--prepare','--json'],env=upgrade)
    if old.returncode==0:
        previous=json.loads(old.stdout)
        current=data('resume','--prepare','--json',env={**upgrade,'LF_BIN':LF})
        records.append({'check':'installed release preparation keeps Session after upgrade','passed':previous['id']==current['id']})
    (ROOT/'metadata.json').write_text(json.dumps({'root':str(ROOT),'endpoint':endpoint,'prepared':prepared,'receipt':receipt}))
except Exception as error:
    records.append({'error':repr(error)})
    print('QA ERROR',repr(error),flush=True)
finally:
    # Include recorded detached owners; they can outlive the CLI that started them.
    pids={p.pid for p in children}
    for db in ROOT.rglob('loopflow.db'):
        try:
            connection=sqlite3.connect(f'file:{db}?mode=ro',uri=True)
            columns=[row[1] for row in connection.execute('pragma table_info(processes)')]
            if 'pid' in columns:
                pids.update(row[0] for row in connection.execute('select pid from processes where pid is not null'))
            connection.close()
        except sqlite3.Error:pass
    ps=subprocess.check_output(['ps','-axo','pid=,ppid='],text=True)
    pairs=[tuple(map(int,line.split())) for line in ps.splitlines()]
    while True:
        added={pid for pid,ppid in pairs if ppid in pids}
        if added <= pids:break
        pids.update(added)
    pids.discard(os.getpid())
    for pid in sorted(pids,reverse=True):
        try:os.kill(pid,signal.SIGTERM)
        except ProcessLookupError:pass
    for child in children:
        try:child.wait(timeout=5)
        except subprocess.TimeoutExpired:child.kill();child.wait()
    for name in ('rex','herdr','lf-server'):
        path=ROOT/(name+'.log')
        if path.exists():records.append({'log':name,'text':path.read_text()[-12000:]})
    records.append({'cleanup':'Stopped private servers and recorded process descendants','root':str(ROOT)})
    (CHECKOUT/'scratch/qa-open-transcript.json').write_text(json.dumps(records,indent=2))
    print('QA root:',ROOT,flush=True)
