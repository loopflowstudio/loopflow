# Subscribe to desktop.view.list, claim a Task mid-stream, report open-view size per reading.
import json, socket, subprocess, sys, time, os
sock_path, repo, lf, task = sys.argv[1:5]
s = socket.socket(socket.AF_UNIX); s.connect(sock_path); f = s.makefile("rwb")
def send(m): f.write((json.dumps(m) + "\n").encode()); f.flush()
send({"type": "hello", "revision": 5, "min_revision": 5,
      "client": {"kind": "desktop", "platform": "macos", "name": "qa", "version": "0.15.3"}})
send({"type": "subscribe", "id": "v", "operation": {"kind": "desktop.view.list", "directory": repo},
      "select": None, "resume": None})
def summarize(body):
    out = {}
    for plan in body:
        ids = {m["work"]["id"] for m in plan["resolution"]["members"]}
        out[plan.get("dashboard_id") or "default"] = (len(ids), task in ids)
    return out
readings = 0; t0 = time.time()
while readings < 2 and time.time() - t0 < 60:
    line = f.readline()
    if not line: break
    m = json.loads(line)
    if m.get("type") != "reading":
        print("msg", m.get("type"), json.dumps(m)[:200]); continue
    frame = m["frame"]; readings += 1
    body = frame.get("body")
    print(f"reading {readings} at {time.time()-t0:.2f}s:", summarize(body) if body is not None else json.dumps(frame)[:300])
    if readings == 1:
        r = subprocess.run([lf, "task", "claim", task], cwd=repo, stdin=subprocess.DEVNULL, capture_output=True, text=True)
        print("claimed", r.returncode, r.stdout.strip()[-80:], "at", f"{time.time()-t0:.2f}s")
s.close()
