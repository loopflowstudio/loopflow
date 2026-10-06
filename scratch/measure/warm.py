import subprocess, json, time, os, sys, threading
home, lf = sys.argv[1], sys.argv[2]
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
def wait(pred):
    while True:
        f = json.loads(p.stdout.readline())
        if pred(f): return f
t = time.time(); wait(lambda f: f["part"] == "planning"); print("first planning %.2fs" % (time.time() - t), flush=True)
sampler = None
for i in range(1, 7):
    if i == 3 and len(sys.argv) > 3:
        sampler = subprocess.Popen(["sample", str(p.pid), "8", "-file", sys.argv[3]], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    t = time.time(); p.stdin.write((json.dumps({"action": "refresh", "id": i}) + "\n").encode()); p.stdin.flush()
    f = wait(lambda f: f["part"] == "planning" and f["answers"] == i)
    print("refresh %d planning %.2fs unavailable=%s" % (i, time.time() - t, f["unavailable"]), flush=True)
hb = wait(lambda f: f["part"] == "heartbeat"); print(hb["body"])
if sampler: sampler.wait()
p.stdin.close(); p.wait()
