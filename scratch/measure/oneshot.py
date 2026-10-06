# One-shot `lf` reads beside a running watch: planning readings and frames each one causes.
# python3 oneshot.py <home> <lf> <cwd of the one-shot read> <count>
import subprocess, json, time, os, sys, threading
home, lf, cwd, n = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
state = {"frames": 0, "readings": 0}
ready = threading.Event()
def read():
    for line in iter(p.stdout.readline, b""):
        f = json.loads(line)
        if f["part"] == "planning": state["frames"] += 1; ready.set()
        if f["part"] == "heartbeat": state["readings"] = f["body"]["projections"].get("planning", 0)
threading.Thread(target=read, daemon=True).start()
ready.wait(); time.sleep(5); before = dict(state)
for _ in range(n):
    subprocess.run([lf, "session", "list", "--json"], cwd=cwd, env=env, capture_output=True); time.sleep(1)
time.sleep(3)
print("%d reads in %s: %d planning readings, %d planning frames" % (n, cwd, state["readings"] - before["readings"], state["frames"] - before["frames"]))
p.kill(); p.wait()
