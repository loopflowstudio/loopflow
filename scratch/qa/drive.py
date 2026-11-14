import os, pty, select, subprocess, sys, time, signal, re
LF = sys.argv[1]
root = "/tmp/lf-qa-887"
env = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "REX_", "CLAUDE_CODE"))}
env.update(HOME=f"{root}/home", LF_HOME=f"{root}/lf", LF_BIN=LF, PS1="qa$ ")
for d in (env["HOME"], env["LF_HOME"], f"{root}/repo"): os.makedirs(d, exist_ok=True)
log = open(f"{root}/transcript.txt", "w")
def note(s): log.write(s + "\n"); log.flush(); print(s, flush=True)

owner = subprocess.Popen([LF, "session", "serve", "--terminal", "--", "/bin/sh"], cwd=f"{root}/repo",
                         env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
first = owner.stdout.readline().strip()
note(f"serve first line: {first!r}")
oid = re.search(r"[0-9a-f-]{8,}|\S+$", first).group(0)

def attach(label, actions, rows=24, cols=80):
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(LF, [LF, "session", "attach", oid], env)
    import fcntl, termios, struct
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    buf = b""
    def read_for(t):
        nonlocal buf
        end = time.time() + t
        while time.time() < end:
            r, _, _ = select.select([fd], [], [], 0.05)
            if r:
                try: chunk = os.read(fd, 65536)
                except OSError: return False
                if not chunk: return False
                buf += chunk
                if b"\x1b[6n" in chunk: os.write(fd, b"\x1b[1;1R")
                if b"\x1b[5n" in chunk: os.write(fd, b"\x1b[0n")
        return True
    t0 = time.time()
    for kind, val in actions:
        if kind == "wait": read_for(val)
        elif kind == "type": os.write(fd, val.encode())
        elif kind == "until":
            end = time.time() + 15
            while val.encode() not in buf and time.time() < end:
                if not read_for(0.1): break
            note(f"[{label}] '{val}' seen={val.encode() in buf} after {time.time()-t0:.2f}s")
    alive = read_for(0)
    text = re.sub(rb"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07\x1b]*(\x07|\x1b\\\\)|\x1b[()][A-Z0-9]|\r", b"", buf).decode(errors="replace")
    note(f"--- [{label}] screen text ---\n{text[-1500:]}\n--- end ---")
    return pid, fd, buf

# 1: attach, run a command, leave a half-typed draft, close the terminal
pid, fd, b1 = attach("first", [("wait", 1.5), ("type", "echo QA_$((40+1))\r"), ("until", "QA_41"), ("type", "echo DRAFT_NOT_SENT"), ("wait", 0.5)])
os.close(fd); os.waitpid(pid, 0)
note(f"owner alive after first view closed: {owner.poll() is None}")
time.sleep(1)
# 2: reattach; screen should show QA_41 history; draft typed into shell line stays as the shell's line (shell owns it), not replayed twice
pid, fd, b2 = attach("second", [("until", "QA_41"), ("type", "\x15echo QA_$((50+2))\r"), ("until", "QA_52"), ("type", "exit\r"), ("wait", 4)], rows=30, cols=100)
try: _, st = os.waitpid(pid, 0); note(f"attach exit status {os.waitstatus_to_exitcode(st)}")
except ChildProcessError: pass
note(f"QA_41 echoed count in second view: {b2.count(b'QA_41')}")
try:
    rc = owner.wait(timeout=10); note(f"owner exited rc={rc}")
except subprocess.TimeoutExpired:
    note("owner still running after exit; killing"); owner.kill(); owner.wait()
note("owner stderr: " + owner.stderr.read()[-500:])
