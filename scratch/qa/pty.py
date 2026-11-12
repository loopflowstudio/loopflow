"""Run argv on a PTY in the background: record output to LOG, answer ESC[6n,
forward lines appended to LOG.in as input. Exits when the child does; writes LOG.exit."""
import os, pty, sys, select, time, struct, fcntl, termios
log = sys.argv[1]; argv = sys.argv[2:]
pid, fd = pty.fork()
if pid == 0:
    os.execvp(argv[0], argv)
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
out = open(log, "ab", buffering=0)
inp = log + ".in"; done = 0
while True:
    r, _, _ = select.select([fd], [], [], 0.1)
    if r:
        try: data = os.read(fd, 65536)
        except OSError:
            _, st = os.waitpid(pid, 0); open(log + ".exit", "w").write(str(os.waitstatus_to_exitcode(st))); break
        if not data:
            _, st = os.waitpid(pid, 0); open(log + ".exit", "w").write(str(os.waitstatus_to_exitcode(st))); break
        out.write(data)
        if b"\x1b[6n" in data: os.write(fd, b"\x1b[1;1R")
    if os.path.exists(inp):
        b = open(inp, "rb").read()
        if len(b) > done:
            os.write(fd, b[done:].replace(b"\\r", b"\r")); done = len(b)
    p, st = os.waitpid(pid, os.WNOHANG)
    if p: 
        try:
            while True:
                d = os.read(fd, 65536)
                if not d: break
                out.write(d)
        except OSError: pass
        open(log + ".exit", "w").write(str(os.waitstatus_to_exitcode(st))); break
