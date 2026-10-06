import subprocess, json, time, os, sys
home, lf, out, secs = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
s = subprocess.Popen(["sample", str(p.pid), secs, "-file", out], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
t = time.time()
while True:
    line = p.stdout.readline()
    if not line: print("reader exited"); break
    if json.loads(line)["part"] == "planning": print("first planning %.2f" % (time.time() - t)); break
s.wait(); p.kill(); p.wait()
