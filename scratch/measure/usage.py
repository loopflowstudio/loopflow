# Usage rows written every 0.2 s for `secs`: how often each part is read meanwhile.
import subprocess, json, time, os, sys, sqlite3, threading
home, lf, secs = sys.argv[1], sys.argv[2], float(sys.argv[3])
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
db = sqlite3.connect(os.path.join(home, "loopflow.db"), timeout=5, isolation_level=None)
wave, session = db.execute("select w.id, s.id from waves w join agent_sessions s on s.wave_id=w.id where w.name='product' order by s.created_at desc limit 1").fetchone()
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
def frames(pred):
    while True:
        f = json.loads(p.stdout.readline())
        if pred(f): return f
p.stdin.write((json.dumps({"action": "scope", "id": 1, "repo": None, "headless": False, "task": None, "wave": wave, "activity": None}) + "\n").encode()); p.stdin.flush()
frames(lambda f: f["part"] == "wave"); frames(lambda f: f["part"] == "planning")
before = frames(lambda f: f["part"] == "heartbeat")["body"]["projections"]
latest = {}
def read():
    while True:
        line = p.stdout.readline()
        if not line: return
        f = json.loads(line)
        if f["part"] == "heartbeat": latest.update(f["body"]["projections"])
threading.Thread(target=read, daemon=True).start()
t = time.time(); n = 0
while time.time() - t < secs:
    db.execute("insert into session_events(session_id,kind,receipt_key,observed_at,payload) values(?,'usage',?,1,'{}')", (session, "measure-%f" % time.time())); n += 1
    time.sleep(0.2)
time.sleep(2.5)
print("%d usage rows in %.0fs; readings:" % (n, secs), {k: latest.get(k, 0) - before.get(k, 0) for k in latest})
p.kill(); p.wait()
