# A file written in a Task checkout, then removed: the write to the planning frame showing it.
import subprocess, json, time, os, sys
home, lf, worktree, n = sys.argv[1], sys.argv[2], os.path.realpath(sys.argv[3]), int(sys.argv[4])
env = {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"], "LF_HOME": home}
p = subprocess.Popen([lf, "monitor", "workspace", "--watch", "--json"], cwd="/Users/jack/src/loopflow", env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
def dirty():
    while True:
        f = json.loads(p.stdout.readline())
        if f["part"] == "heartbeat": last = f["body"]["projections"]; continue
        if f["part"] != "planning": continue
        for w in f["body"]["roadmap"]["waves"]:
            for t in (w["tasks"].get("items") or []):
                ws = t["reference"]["workspace"]
                if ws and os.path.realpath(ws["worktree"]) == worktree:
                    return t["condition"]["local_progress"]["dirty"]
        sys.exit("no Task uses " + worktree)
assert dirty() is False, "checkout must start clean"
probe = os.path.join(worktree, "lf-measure-probe.txt")
for i in range(n):
    time.sleep(4)
    t = time.time(); open(probe, "w").write("x")
    while dirty() is not True: pass
    shown = time.time() - t
    time.sleep(4)
    t = time.time(); os.remove(probe)
    while dirty() is not False: pass
    print("dirty %.2fs clean %.2fs" % (shown, time.time() - t), flush=True)
p.kill(); p.wait()
