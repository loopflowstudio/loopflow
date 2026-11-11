# pty.py <out> lf-args... : run in a PTY, answer cursor queries, save transcript.
import os, pty, select, sys, time
out = open(sys.argv[1], 'wb')
pid, fd = pty.fork()
if pid == 0:
    os.execvp(sys.argv[2], sys.argv[2:])
deadline = time.time() + float(os.environ.get("QA_SECS", "110"))
while time.time() < deadline:
    r, _, _ = select.select([fd], [], [], 0.2)
    if r:
        try: data = os.read(fd, 65536)
        except OSError: break
        if not data: break
        out.write(data); out.flush()
        if b'\x1b[6n' in data: os.write(fd, b'\x1b[1;1R')
    done, status = os.waitpid(pid, os.WNOHANG)
    if done: break
else:
    os.kill(pid, 9)
    done = 0
if not done:
    _, status = os.waitpid(pid, 0)
out.write(f'\n[exit {os.waitstatus_to_exitcode(status)}]\n'.encode())
