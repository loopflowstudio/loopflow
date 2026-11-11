# pty_input.py <out> <secs:text>... -- cmd...: run in a PTY, type each text at its time, answer cursor queries.
import os, pty, select, sys, time
args = sys.argv[1:]; out = open(args.pop(0), 'wb'); sep = args.index('--')
inputs = sorted((float(a.split(':', 1)[0]), a.split(':', 1)[1]) for a in args[:sep]); cmd = args[sep + 1:]
pid, fd = pty.fork()
if pid == 0:
    os.execvp(cmd[0], cmd)
start = time.time(); deadline = start + float(os.environ.get("QA_SECS", "60")); status = None
while time.time() < deadline:
    while inputs and time.time() - start >= inputs[0][0]:
        text = inputs.pop(0)[1].replace('\\r', '\r')
        for ch in text: os.write(fd, ch.encode()); time.sleep(0.12)
    r, _, _ = select.select([fd], [], [], 0.2)
    if r:
        try: data = os.read(fd, 65536)
        except OSError: break
        if not data: break
        out.write(data); out.flush()
        if b'\x1b[6n' in data: os.write(fd, b'\x1b[1;1R')
    done, st = os.waitpid(pid, os.WNOHANG)
    if done: status = st; break
if status is None:
    os.kill(pid, 9); _, status = os.waitpid(pid, 0)
out.write(f'\n[exit {os.waitstatus_to_exitcode(status)}]\n'.encode())
