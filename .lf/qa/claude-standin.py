#!/usr/bin/python3 -I
# Stand-in `claude` for .lf/qa/task-children.md: answers each line, runs a "!cmd" line as a Bash tool call,
# and runs the hooks lf passed in --settings as installed. Calls and hook output log to $CLAUDE_QA_LOG.
import json, os, re, subprocess, sys
LOG = os.environ.get("CLAUDE_QA_LOG", os.devnull)
args = sys.argv[1:]
if args[:1] == ["--version"]:
    print("2.1.328 (Claude Code)"); sys.exit(0)
log = open(LOG, "a")
log.write(json.dumps({"cwd": os.getcwd(), "tty": [sys.stdin.isatty(), sys.stdout.isatty()]}) + "\n"); log.flush()
def opt(*names):
    for n in names:
        if n in args: return args[args.index(n) + 1]
sid = opt("--session-id") or opt("--resume", "-r")
new = "--session-id" in args
settings = opt("--settings")
hooks = {}
if settings:
    try: hooks = (json.load(open(settings)) if os.path.exists(settings) else json.loads(settings)).get("hooks", {})
    except Exception as e: log.write("settings: %r\n" % e)
enc = re.sub(r"[/.]", "-", os.getcwd())
tdir = os.path.join(os.environ.get("CLAUDE_CONFIG_DIR", os.path.expanduser("~/.claude")), "projects", enc)
os.makedirs(tdir, exist_ok=True)
tpath = os.path.join(tdir, "%s.jsonl" % sid)
def found():
    import glob
    return glob.glob(os.path.join(os.path.dirname(tdir), "*", "%s.jsonl" % sid))
def hook(event, matcher_value=None, **extra):
    out = []
    for group in hooks.get(event, []):
        m = group.get("matcher")
        if m and matcher_value is not None and not re.fullmatch(m, matcher_value): continue
        if m and matcher_value is None and event == "SessionStart": continue
        for h in group.get("hooks", []):
            payload = dict(session_id=sid, transcript_path=tpath, cwd=os.getcwd(), hook_event_name=event, **extra)
            r = subprocess.run(h["command"], shell=True, input=json.dumps(payload), capture_output=True, text=True)
            log.write(json.dumps({"hook": event, "cmd": h["command"], "rc": r.returncode, "out": r.stdout, "err": r.stderr}) + "\n"); log.flush()
            if r.stdout.strip(): out.append(r.stdout.strip())
    for o in out: print("[hook %s] %s" % (event, o), flush=True)
def note(kind, text):
    open(tpath, "a").write(json.dumps({"type": kind, "sessionId": sid, "message": {"role": kind, "content": text}}) + "\n")
if new: print("started %s" % sid, flush=True)
else:
    if not found(): print("No conversation found with session ID: %s" % sid, file=sys.stderr); sys.exit(1)
    print("resumed %s in %s" % (sid, os.getcwd()), flush=True)
hook("SessionStart", "startup" if new else "resume", source="startup" if new else "resume")
def answer(stream):
    for line in stream:
        line = line.rstrip("\n")
        if line == "quit": sys.exit(0)
        if not line: continue
        hook("UserPromptSubmit", prompt=line)
        note("user", line)
        if line.startswith("!"):
            hook("PreToolUse", "Bash", tool_name="Bash", tool_input={"command": line[1:]})
            r = subprocess.run(line[1:], shell=True, capture_output=True, text=True, stdin=subprocess.DEVNULL)
            print(r.stdout + r.stderr, end="", flush=True); print("[exit %d]" % r.returncode, flush=True)
            hook("PostToolUse", "Bash", tool_name="Bash", tool_input={"command": line[1:]}, tool_response={"stdout": r.stdout})
        else:
            print("reply %s: %s" % (sid[:8], line[:60]), flush=True)
        note("assistant", "ok")
        hook("Stop", stop_hook_active=False)
answer(sys.stdin)
if sys.stdout.isatty() and not sys.stdin.isatty():
    answer(open("/dev/tty"))
