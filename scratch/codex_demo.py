#!/usr/bin/env python3
"""Headless demo for 430c74: Codex keeps working while lf's store and observer are gone."""

import fcntl, http.server, json, os, pty, select, signal, sqlite3, struct, subprocess, sys, termios, threading, time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests/e2e"))
from codex_terminal import Model  # noqa: E402

LF = Path(sys.argv[1]).resolve()
CODEX = Path(sys.argv[2]).resolve()
ROOT = Path(sys.argv[3]).resolve()


class Pty:
    def __init__(self, args, env, cwd, transcript):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

        def own():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        self.child = subprocess.Popen(args, env=env, cwd=cwd, stdin=slave, stdout=slave, stderr=slave, preexec_fn=own)
        os.close(slave)
        self.out = bytearray()
        self.transcript = transcript

    def pump(self, seconds):
        end = time.monotonic() + seconds
        while time.monotonic() < end and self.child.poll() is None:
            if select.select([self.master], [], [], 0.1)[0]:
                try:
                    chunk = os.read(self.master, 65536)
                except OSError:
                    return
                self.out.extend(chunk)
                if b"\x1b[6n" in chunk:
                    os.write(self.master, b"\x1b[1;1R")

    def wait_for(self, needle, seconds=30, start=0):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if needle in self.out[start:]:
                return True
            if self.child.poll() is not None:
                break
            self.pump(0.2)
        self.transcript.write_bytes(self.out)
        raise AssertionError(f"never saw {needle!r}; see {self.transcript}")

    def type(self, text):
        os.write(self.master, b"\x1b[200~" + text.encode() + b"\x1b[201~")
        self.pump(0.4)
        os.write(self.master, b"\r")

    def command(self, text):
        os.write(self.master, text.encode())
        self.pump(0.4)
        os.write(self.master, b"\r")

    def exit(self):
        self.pump(1)
        self.command("/exit")
        end = time.monotonic() + 20
        while self.child.poll() is None and time.monotonic() < end:
            self.pump(0.2)
        self.pump(0.3)
        self.transcript.write_bytes(self.out)
        if self.child.poll() is None:
            os.killpg(self.child.pid, signal.SIGKILL)
            raise AssertionError("did not exit")
        return self.child.returncode


def main():
    ROOT.mkdir(parents=True)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    repo, home, native, bin_dir, lf_home = (ROOT / n for n in ["repo", "home", "codex", "bin", "lf"])
    for d in [repo, home, native, bin_dir]:
        d.mkdir()
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / "scratch").mkdir()
    skill = repo / ".lf/skills/native-probe.md"
    skill.parent.mkdir(parents=True)
    skill.write_text("NATIVE_SKILL_STARTUP: inspect the workspace with a tool, then answer.\n")
    (native / "config.toml").write_text(f"""cli_auth_credentials_store="file"
model="fixture"
model_provider="fixture"
[model_providers.fixture]
name="fixture"
base_url="http://127.0.0.1:{server.server_port}/v1"
wire_api="responses"
requires_openai_auth=false
[projects.{json.dumps(str(repo))}]
trust_level="trusted"
""")
    (bin_dir / "codex").write_text(f"#!/bin/sh\nexec {CODEX} --no-alt-screen \"$@\"\n")
    (bin_dir / "codex").chmod(0o755)
    env = {"HOME": str(home), "CODEX_HOME": str(native), "LF_HOME": str(lf_home), "LF_BIN": str(LF),
           "PATH": f"{bin_dir}:/usr/bin:/bin", "TERM": "xterm-256color", "RUST_LOG": "warn"}
    db = lf_home / "loopflow.db"
    aside = ROOT / "store-aside"
    aside.mkdir()

    print("1. start a Codex Session; first turn runs a tool and answers", flush=True)
    t = Pty([str(LF), "--diff", "none", "-a", "codex", "native-probe"], env, repo, ROOT / "1-launch.txt")
    t.wait_for(b"FIXTURE_DONE", 40)
    t.wait_for(b"fixture default")
    t.pump(1.5)
    print("   saw FIXTURE_DONE", flush=True)

    print("2. move loopflow.db away while the Session is open; type, reply, tool", flush=True)
    for name in ["loopflow.db", "loopflow.db-wal", "loopflow.db-shm"]:
        if (lf_home / name).exists():
            (lf_home / name).rename(aside / name)
    # Leave a directory in its place so no process can reopen or recreate the store.
    db.mkdir()
    mark = len(t.out)
    t.type("typed reconnect follow-up while the store is gone")
    t.wait_for(b"FOLLOWUP_DONE", 40, mark)
    tool_requests = [r for r in Model.requests if b"tool-followup" in r and b"function_call_output" in r]
    assert tool_requests, "tool output never reached the model"
    print("   saw FOLLOWUP_DONE after its tool ran", flush=True)
    status = t.exit()
    left = bytes(t.out[-4000:])
    print(f"   exit status {status}; 'lf resume' printed: {b'lf resume' in left}", flush=True)
    assert status == 0, status

    print("3. restore the store; history and resume", flush=True)
    db.rmdir()
    for p in aside.iterdir():
        p.rename(lf_home / p.name)
    with sqlite3.connect(db) as conn:
        session_id = conn.execute("SELECT session_id FROM session_events WHERE kind='observed' LIMIT 1").fetchone()[0]
        outputs = conn.execute("SELECT json_extract(payload,'$.text') FROM session_events WHERE session_id=? AND kind='output' ORDER BY seq", (session_id,)).fetchall()
    print(f"   session {session_id}; recorded outputs before resume: {outputs}", flush=True)

    t = Pty([str(LF), "session", "resume", session_id], env, repo, ROOT / "3-resume.txt")
    t.wait_for(b"FOLLOWUP_DONE", 40)
    t.wait_for(b"fixture default")
    t.pump(2)
    print("   resume shows the turn typed while the store was gone", flush=True)

    print("4. kill the observer while idle; the TUI keeps working", flush=True)
    ps = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    observers = [l.split(None, 1) for l in ps.splitlines() if "__codex-observe" in l and str(LF) in l]
    observers = [(int(p), c) for p, c in observers]
    # Verify ownership through the process environment root: only this demo's LF_HOME.
    owned = []
    for pid, cmd in observers:
        e = subprocess.run(["ps", "-E", "-p", str(pid), "-o", "command="], capture_output=True, text=True).stdout
        if str(lf_home) in e or session_id in cmd:
            owned.append(pid)
    assert len(owned) == 1, (observers, owned)
    os.kill(owned[0], signal.SIGKILL)
    print(f"   killed observer pid {owned[0]}", flush=True)
    t.pump(1.5)
    mark = len(t.out)
    t.type("AFTER_NEW_PROMPT with no observer")
    t.wait_for(b"NEW_THREAD_DONE", 40, mark)
    print("   saw NEW_THREAD_DONE with the observer dead", flush=True)
    status = t.exit()
    print(f"   exit status {status}", flush=True)
    assert status == 0, status
    with sqlite3.connect(db) as conn:
        live = conn.execute("SELECT count(*) FROM processes WHERE kind='agent' AND completed_at IS NULL").fetchone()[0]
    ps = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    leftovers = [l for l in ps.splitlines() if str(native) in l or str(lf_home) in l]
    print(f"   live agent rows {live}; leftover demo processes {leftovers}", flush=True)
    server.shutdown()


if __name__ == "__main__":
    main()
