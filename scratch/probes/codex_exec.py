import http.server, json, os, signal, subprocess, sys, threading, time
from pathlib import Path
root = Path(__file__).parent
CODEX = str(Path.home()/".local/bin/codex")
class Model(http.server.BaseHTTPRequestHandler):
    requests = []
    def log_message(self,*a): pass
    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length",0)))
        body = json.loads(raw); self.requests.append(body)
        text = json.dumps(body.get("input",[]))
        if "LIMIT_PROMPT" in text:
            data = json.dumps({"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":int(time.time())+3600}}).encode()
            self.send_response(429); self.send_header("Content-Type","application/json")
            self.send_header("Content-Length",str(len(data))); self.end_headers(); self.wfile.write(data); return
        done = any(i.get("type")=="function_call_output" for i in body.get("input",[]) if i.get("call_id","").startswith("t-"))
        # only count outputs after last user msg
        items = body.get("input",[])
        last_user = max([n for n,i in enumerate(items) if i.get("type")=="message" and i.get("role")=="user"] or [0])
        done = any(i.get("type")=="function_call_output" for i in items[last_user:])
        cmd = "touch started; sleep 6; touch finished" if "INTERRUPT_PROMPT" in json.dumps(items[last_user]) else "/bin/echo TOOL_RAN"
        if done:
            item = {"id":"msg_f","type":"message","role":"assistant","content":[{"type":"output_text","text":"FIXTURE_ANSWER"}]}
        else:
            item = {"id":"fc_1","type":"function_call","name":"exec_command","call_id":f"t-{len(self.requests)}","arguments":json.dumps({"cmd":cmd})}
        resp = {"id":"resp_f","status":"in_progress","output":[]}
        evs = [("response.created",{"response":resp}),("response.output_item.added",{"output_index":0,"item":item}),
               ("response.output_item.done",{"output_index":0,"item":item}),
               ("response.completed",{"response":{**resp,"status":"completed","output":[item],"usage":{"input_tokens":20,"output_tokens":5,"total_tokens":25}}})]
        self.send_response(200); self.send_header("Content-Type","text/event-stream"); self.end_headers()
        for k,d in evs:
            self.wfile.write(f"event: {k}\ndata: {json.dumps({'type':k,**d})}\n\n".encode()); self.wfile.flush()
srv = http.server.ThreadingHTTPServer(("127.0.0.1",0),Model)
threading.Thread(target=srv.serve_forever,daemon=True).start()
home, native, work = root/"home", root/"codex", root/"work"
for d in (home,native,work): d.mkdir(exist_ok=True)
subprocess.run(["git","init","-q",str(work)],check=True)
(native/"config.toml").write_text(f'''cli_auth_credentials_store="file"
model="fixture"
model_provider="fixture"
[model_providers.fixture]
name="fixture"
base_url="http://127.0.0.1:{srv.server_port}/v1"
wire_api="responses"
requires_openai_auth=false
request_max_retries=0
stream_max_retries=0
[projects.{json.dumps(str(work))}]
trust_level="trusted"
''')
hook = root/"hook.sh"
hook.write_text(f'''#!/bin/sh
cat >> {root}/hook-$1.log; echo >> {root}/hook-$1.log
case $1 in post) echo '{{"hookSpecificOutput":{{"hookEventName":"PostToolUse","additionalContext":"STEER_MARKER_7781"}}}}';;
start) echo '{{"hookSpecificOutput":{{"hookEventName":"SessionStart","additionalContext":"START_MARKER_4410"}}}}';; esac
''')
hook.chmod(0o755)
hooks = ('hooks={SessionStart=[{hooks=[{type="command",command="%s start",timeout=30}]}],'
         'PostToolUse=[{hooks=[{type="command",command="%s post",timeout=30}]}],'
         'Stop=[{hooks=[{type="command",command="%s stop",timeout=30}]}]}') % (hook,hook,hook)
env = {"HOME":str(home),"CODEX_HOME":str(native),"PATH":"/usr/bin:/bin","TERM":"dumb"}
base = [CODEX,"exec","--json","--dangerously-bypass-approvals-and-sandbox","--dangerously-bypass-hook-trust","--skip-git-repo-check","-C",str(work),"-c",hooks]
def run(args, prompt, kill=None):
    Model.requests.clear()
    for f in ("started","finished"): (work/f).unlink(missing_ok=True)
    p = subprocess.Popen(args,env=env,cwd=work,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
    p.stdin.write(prompt); p.stdin.close()
    if kill:
        t=time.monotonic()+20
        while not (work/"started").exists() and time.monotonic()<t and p.poll() is None: time.sleep(.05)
        os.killpg(p.pid,kill)
    try: p.wait(timeout=40)
    except subprocess.TimeoutExpired: os.killpg(p.pid,signal.SIGKILL); p.wait()
    out,err = p.stdout.read(), p.stderr.read()
    late=False
    if kill:
        t=time.monotonic()+8
        while time.monotonic()<t:
            if (work/"finished").exists(): late=True;break
            time.sleep(.1)
    return p.returncode,out,err,late
def show(name,rc,out,err):
    print(f"== {name} exit={rc}")
    for line in out.splitlines(): print("  ",line[:400])
    if err.strip(): print("   STDERR:",err.strip()[-600:])
rc,out,err,_ = run(base+["-"],"NORMAL_PROMPT one"); show("normal",rc,out,err)
reqs=list(Model.requests)
print("requests",len(reqs),"start_marker",["START_MARKER_4410" in json.dumps(r) for r in reqs],"steer_marker",["STEER_MARKER_7781" in json.dumps(r) for r in reqs])
tid = next((json.loads(l).get("thread_id") for l in out.splitlines() if l.startswith("{") and "thread_id" in l),None)
print("thread",tid)
if tid:
    rc,out,err,_ = run([CODEX,"exec","resume",tid,"--json","--dangerously-bypass-approvals-and-sandbox","--dangerously-bypass-hook-trust","--skip-git-repo-check","-c",hooks,"-"],"SECOND_PROMPT two"); show("resume",rc,out,err)
    print("resume carries first turn:",["NORMAL_PROMPT one" in json.dumps(r) for r in Model.requests], "same thread:", tid in out)
for sig in (signal.SIGTERM, signal.SIGINT):
    rc,out,err,late = run(base+["-"],"INTERRUPT_PROMPT",kill=sig); show(f"interrupt {sig.name}",rc,out,err)
    print("started",(work/"started").exists(),"finished_late",late)
    roll = sorted(native.rglob("rollout-*.jsonl"),key=lambda p:p.stat().st_mtime)[-1]
    lines = roll.read_text().splitlines()
    print("rollout tail:",[ (json.loads(l).get("type"), (json.loads(l).get("payload") or {}).get("type")) for l in lines[-6:]])
    print("rollout mentions abort/interrupt:", any(("abort" in l or "interrupt" in l) for l in lines))
rc,out,err,_ = run(base+["-"],"LIMIT_PROMPT"); show("limit",rc,out,err)
for f in sorted(root.glob("hook-*.log")): print(f.name, len(f.read_text().splitlines()),"lines")
