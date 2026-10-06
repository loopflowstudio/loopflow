# Commit-to-frame: sqlite3 renames a Task, then a Session, on a private Home
# copy; each is timed from the writer's exit to the frame that carries it.
import subprocess, json, time, os, sys, threading, queue
home, lf, samples = sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 5
repo, db = "/Users/jack/src/loopflow", home + "/loopflow.db"
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd=repo, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
frames = queue.Queue()
threading.Thread(target=lambda: [frames.put((time.time(), l)) for l in p.stdout], daemon=True).start()
def wait(pred):
    while True:
        at, line = frames.get(timeout=120)
        if pred(line): return at
def sql(q):
    r = subprocess.run(["sqlite3", db, ".timeout 5000", q], capture_output=True, text=True, check=True)
    return r.stdout.strip()
p.stdin.write((json.dumps({"action": "scope", "id": 1, "repo": repo, "headless": False, "task": None, "wave": None, "activity": None}) + "\n").encode()); p.stdin.flush()
first = set()
wait(lambda l: first.update(n for n in (b"planning", b"sessions") if b'"part":"' + n + b'"' in l) or len(first) == 2)
item = sql("select id from pm_items where identifier='LOO-382' limit 1")
session = sql("select id from agent_sessions where repo like '%/loopflow' and completed_at is null and interactive=1 order by created_at desc limit 1")
for i in range(samples):
    time.sleep(1.5)
    while not frames.empty(): frames.get()
    name = "frame probe %d %d" % (os.getpid(), i)
    sql("UPDATE pm_items SET body=json_set(body,'$.name','%s') WHERE id='%s'" % (name, item)); t = time.time()
    task = wait(lambda l: b'"part":"planning"' in l and name.encode() in l) - t
    time.sleep(1.5)
    while not frames.empty(): frames.get()
    sql("UPDATE agent_sessions SET title='%s' WHERE id='%s'" % (name, session)); t = time.time()
    row = wait(lambda l: b'"part":"sessions"' in l and name.encode() in l) - t
    print("sample %d  Task %.0f ms  Session %.0f ms" % (i, task * 1000, row * 1000), flush=True)
p.kill(); p.wait()
