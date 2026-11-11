# Daemonized PTY driver: runs argv in cwd, logs output, writes lines from a cmd file.
import os, sys, pty, time, select
cwd, log, cmdfile = sys.argv[1], sys.argv[2], sys.argv[3]
argv = sys.argv[4:]
if os.fork(): sys.exit(0)
os.setsid()
if os.fork(): os._exit(0)
pid, fd = pty.fork()
if pid == 0:
    os.chdir(cwd)
    os.environ['TERM'] = 'xterm-256color'
    for k in list(os.environ):
        if k.startswith('LF_') or k.startswith('CLAUDE_CODE') or k == 'CLAUDECODE':
            del os.environ[k]
    os.execvp(argv[0], argv)
import fcntl, termios, struct
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 50, 160, 0, 0))
out = open(log, 'ab')
open(log + '.pid', 'w').write(str(pid))
seen = 0
while True:
    r, _, _ = select.select([fd], [], [], 0.5)
    if r:
        try: data = os.read(fd, 65536)
        except OSError: break
        if not data: break
        out.write(data); out.flush()
        if b'\x1b[6n' in data: os.write(fd, b'\x1b[1;1R')
    try:
        lines = open(cmdfile).read().split('\n')
    except FileNotFoundError:
        lines = []
    for line in lines[seen:-1] if lines else []:
        if line == '<CR>': os.write(fd, b'\r')
        elif line == '<ESC>': os.write(fd, b'\x1b')
        else:
            os.write(fd, line.encode()); time.sleep(0.3); os.write(fd, b'\r')
        time.sleep(0.2)
    if lines: seen = max(seen, len(lines) - 1)
    try:
        p, _ = os.waitpid(pid, os.WNOHANG)
        if p: break
    except ChildProcessError: break
out.write(b'\n[driver: child exited]\n')
