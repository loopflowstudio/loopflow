# CPU and memory of a reader left alone: python3 idle.py <home> <lf> <secs> <monitor subcommand>
import subprocess, json, time, os, sys
home, lf, secs, sub = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
t = time.time()
p = subprocess.Popen([lf, "monitor", sub, "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
p.stdout.readline(); first = time.time() - t
def ps():
    out = subprocess.run(["ps", "-o", "time=,rss=", "-p", str(p.pid)], capture_output=True, text=True).stdout.split()
    m, s = out[0].split(":"); return float(m) * 60 + float(s), int(out[1]) / 1024
import threading
threading.Thread(target=lambda: [None for _ in iter(p.stdout.readline, b"")], daemon=True).start()
time.sleep(10); a, _ = ps(); time.sleep(secs); b, rss = ps()
print("%s: first frame %.2fs, %.1f%% of a core over %.0fs, %.0f MB" % (sub, first, 100 * (b - a) / secs, secs, rss))
p.kill(); p.wait()
