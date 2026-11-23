#!/usr/bin/python3 -I
# pty-drive.py <steps.json> <cmd...>: run cmd in a 40x120 PTY and print its screen text.
# Steps: ["wait", regex, secs] | ["send", line] | ["key", bytes] | ["sleep", secs].
import json, os, pty, re, select, sys, time
steps = json.loads(sys.argv[1]); cmd = sys.argv[2:]
pid, fd = pty.fork()
import fcntl, struct, termios
if pid != 0: fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
if pid == 0:
    time.sleep(0.2)
    os.execvp(cmd[0], cmd)
buf = b""
def drain(t):
    global buf
    end = time.time() + t
    while True:
        left = end - time.time()
        r, _, _ = select.select([fd], [], [], max(left, 0))
        if not r: return
        try: d = os.read(fd, 65536)
        except OSError: return
        if not d: return
        buf += d
        for q, a in ((b"\x1b[6n", b"\x1b[1;1R"), (b"\x1b[5n", b"\x1b[0n")):
            if q in d: os.write(fd, a)
        if left <= 0: return
mark = 0
for s in steps:
    if s[0] == "key": os.write(fd, s[1].encode()); drain(0.5)
    elif s[0] == "send": os.write(fd, s[1].encode() + b"\r"); drain(0.3)
    elif s[0] == "sleep": drain(s[1])
    elif s[0] == "wait":
        end = time.time() + s[2]
        while time.time() < end:
            m = re.search(s[1].encode(), buf[mark:])
            if m: mark += m.end(); break
            drain(0.2)
        else: buf += ("\n[pty: timed out waiting for %s]\n" % s[1]).encode()
drain(0.5)
text = buf.decode(errors="replace")
text = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>()][A-Za-z0-9]?", "", text).replace("\r", "")
print(text)
try: os.kill(pid, 15)
except OSError: pass
