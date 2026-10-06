import subprocess, json, time, os, sys
home, lf, out = sys.argv[1], sys.argv[2], sys.argv[3]
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
s = subprocess.Popen(["sample", str(p.pid), "12", "-file", out], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
t = time.time()
while True:
    f = json.loads(p.stdout.readline())
    if f["part"] == "planning": break
print("first planning %.2fs" % (time.time() - t)); s.wait(); p.stdin.close(); p.wait()
