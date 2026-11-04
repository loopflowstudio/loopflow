#!/usr/bin/env python3
"""LOO-725 headless demo walkthrough: demo.md's two roots, copied lf, stand-in provider.

Touches only the two roots below (plus one relink of this checkout's target/debug/lf).
Signals only PIDs read from the private store and verified against the copied binary.
"""
import collections
import fcntl
import json
import os
import pathlib
import pty
import re
import shutil
import signal
import sqlite3
import struct
import subprocess
import sys
import termios
import threading
import time

ID = sys.argv[1] if len(sys.argv) > 1 else "loo-725-walkthrough"
GRACE = 3
REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
BUILT = REPO_ROOT / "target/debug/lf"
MACHINE = pathlib.Path.home() / ".lf-machine"
DURABLE = MACHINE / "demo-history" / ID
RUNTIME = MACHINE / "demos" / ID
REOPEN_RUNTIME = MACHINE / "demos" / f"{ID}-reopen"
HOME = DURABLE / "home"
REPO = DURABLE / "repo"
PROVIDER_BIN = DURABLE / "provider"
EVIDENCE = DURABLE / "evidence"
PARKED = MACHINE / "demo-history" / f"{ID}.moved"
STORE_FILES = ["loopflow.db", "loopflow.db-wal", "loopflow.db-shm"]
PRESSURE_CPU = 200  # scripts/test.py's host-security threshold, sustained 15 seconds
PRESSURE_SECONDS = 15

STAND_IN = r"""#!/opt/homebrew/bin/python3
import json, os, pathlib, subprocess, sys, tty
if sys.argv[1:] == ["--version"]:
    print("2.1.317")
    sys.exit(0)
home = pathlib.Path(os.environ["HOME"])
with open(home / "launches", "a") as log:
    log.write(json.dumps({"pid": os.getpid(), "ppid": os.getppid(), "argv": sys.argv[1:]}) + "\n")
def value(flag):
    return sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else None
resumed = value("--resume")
native = resumed or value("--session-id") or "standin"
settings = json.loads(pathlib.Path(value("--settings")).read_text())
def hook(event, **extra):
    command = settings["hooks"][event][0]["hooks"][0]["command"]
    payload = dict(session_id=native, hook_event_name=event, **extra)
    subprocess.run(command, shell=True, input=json.dumps(payload).encode(), stdout=subprocess.DEVNULL, check=False)
hook("SessionStart", source="resume" if resumed else "startup")
if not resumed:
    hook("UserPromptSubmit", prompt_id="seed", prompt="initial")
    hook("Stop", prompt_id="seed", background_tasks=[], session_crons=[])
terminal = os.open("/dev/tty", os.O_RDWR)
tty.setraw(terminal)
os.write(1, (b"EDITOR_RESUMED\r\n" if resumed else b"EDITOR_READY\r\n"))
draft = bytearray()
while True:
    key = os.read(terminal, 1)
    if key in (b"\x04", b""):
        break
    if key == b"\x7f":
        draft = draft[:-1]
    elif key == b"\r":
        prompt = draft.decode()
        hook("UserPromptSubmit", prompt_id="typed", prompt=prompt)
        hook("Stop", prompt_id="typed", background_tasks=[], session_crons=[])
        with open(home / "submitted", "a") as log:
            log.write(prompt + "\n")
        draft.clear()
    else:
        draft.extend(key)
    os.write(1, b"\r\nEDITOR:" + draft + b"\r\n")
"""


def private_env(binary: pathlib.Path) -> dict:
    return {
        "PATH": f"{PROVIDER_BIN}:{binary.parent}:/usr/bin:/bin",
        "HOME": str(HOME),
        "LF_HOME": str(HOME),
        "LF_BIN": str(binary),
        "CLAUDE_CONFIG_DIR": str(HOME / "claude"),
        "CODEX_HOME": str(HOME / "codex"),
        "TERM": "xterm-256color",
    }


def run_private(binary: pathlib.Path, *args: str, timeout: int = 30) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(binary), *args], cwd=REPO, env=private_env(binary), stdin=subprocess.DEVNULL,
        capture_output=True, text=True, timeout=timeout,
    )


class Term:
    """One real lf viewer on a PTY."""

    def __init__(self, binary: pathlib.Path, *args: str, name: str):
        self.buffer = bytearray()
        self.log = open(EVIDENCE / f"term-{name}.log", "wb")
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(REPO)
            os.execve(str(binary), [str(binary), *args], private_env(binary))
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        while True:
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                return
            if not data:
                return
            self.buffer.extend(data)
            self.log.write(data)
            self.log.flush()

    def send(self, text: str):
        os.write(self.fd, text.encode())

    def saw(self, needle: str) -> bool:
        return needle.encode() in self.buffer

    def wait(self, needle: str, timeout: float = 20) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.saw(needle):
                return True
            time.sleep(0.05)
        return False

    def close(self):
        """Hang up as a closing terminal does; os.close blocks behind the reader thread."""
        try:
            os.kill(self.pid, signal.SIGHUP)
        except ProcessLookupError:
            pass
        deadline = time.time() + 5
        while time.time() < deadline:
            pid, _ = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                break
            time.sleep(0.05)
        os.close(self.fd)


def alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def command_of(pid: int) -> str:
    return subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()


def fds(pid: int) -> int:
    out = subprocess.run(["lsof", "-n", "-P", "-p", str(pid)], capture_output=True, text=True).stdout
    return max(len(out.splitlines()) - 1, 0)


class Sampler(threading.Thread):
    """One-second CPU/FD/process samples, grouped by phase."""

    def __init__(self):
        super().__init__(daemon=True)
        self.phase = "baseline"
        self.owner = None
        self.rows = []
        self.lf_pids = collections.defaultdict(set)
        self.stopped = threading.Event()
        self.spd = int(subprocess.run(["pgrep", "-x", "syspolicyd"], capture_output=True, text=True).stdout.split()[0])

    def run(self):
        while not self.stopped.is_set():
            table = subprocess.run(["ps", "-axo", "pid=,pcpu=,command="], capture_output=True, text=True).stdout
            cpu = {}
            for line in table.splitlines():
                parts = line.split(None, 2)
                if len(parts) < 3:
                    continue
                pid, pcpu, command = int(parts[0]), float(parts[1]), parts[2]
                cpu[pid] = pcpu
                if str(RUNTIME) in command or str(PROVIDER_BIN) in command:
                    self.lf_pids[self.phase].add(pid)
            row = {"t": time.time(), "phase": self.phase, "syspolicyd_cpu": cpu.get(self.spd)}
            if self.owner and self.owner in cpu:
                row["owner_cpu"] = cpu[self.owner]
                row["owner_fds"] = fds(self.owner)
            self.rows.append(row)
            time.sleep(1)

    def under_pressure(self) -> bool:
        recent = [r["syspolicyd_cpu"] or 0 for r in self.rows[-PRESSURE_SECONDS:]]
        return len(recent) == PRESSURE_SECONDS and min(recent) >= PRESSURE_CPU

    def set(self, phase: str):
        if self.under_pressure():
            raise RuntimeError(f"host-security pressure before {phase}: stopping the walkthrough")
        self.phase = phase
        marks.append((phase, time.time(), lf_log_size()))
        print(f"[{time.strftime('%H:%M:%S')}] {phase}", flush=True)


marks = []


def lf_log_size() -> int:
    path = HOME / "logs/lf.log"
    return path.stat().st_size if path.exists() else 0


def session_facts() -> dict:
    db = sqlite3.connect(f"file:{HOME}/loopflow.db?mode=ro", uri=True, timeout=5)
    row = db.execute(
        "SELECT s.id, s.agent_process_id, s.transport_lf_process_id FROM agent_sessions s "
        "WHERE s.transport_lf_process_id IS NOT NULL ORDER BY s.created_at DESC LIMIT 1"
    ).fetchone()
    pid = lambda process: db.execute("SELECT pid FROM processes WHERE id=?1", (process,)).fetchone()
    native = db.execute("SELECT native_input FROM processes WHERE id=?1", (row[2],)).fetchone()
    return {"session": row[0], "provider_process": row[1], "owner_process": row[2],
            "provider_pid": (pid(row[1]) or [None])[0], "owner_pid": (pid(row[2]) or [None])[0],
            "owner_native_input": native[0] if native else None}


def launches() -> list:
    path = HOME / "launches"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def root_holders(root: pathlib.Path) -> list:
    out = subprocess.run(["lsof", "-n", "-P", "+D", str(root)], capture_output=True, text=True).stdout
    return sorted({line.split()[1] for line in out.splitlines()[1:]})


def stop_verified(pid: int | None, results: dict):
    command = command_of(pid) if pid else ""
    if pid and alive(pid) and any(str(root) in command for root in (RUNTIME, REOPEN_RUNTIME, DURABLE)):
        results.setdefault("forced_stops", []).append(pid)
        os.kill(pid, signal.SIGTERM)


def summarize(sampler: Sampler) -> dict:
    summary = {}
    for phase, rows in _group(sampler.rows).items():
        cpu = [r["syspolicyd_cpu"] for r in rows if r["syspolicyd_cpu"] is not None]
        owner = [r["owner_cpu"] for r in rows if "owner_cpu" in r]
        owner_fds = [r["owner_fds"] for r in rows if "owner_fds" in r]
        summary[phase] = {
            "seconds": len(rows),
            "syspolicyd_cpu_max": max(cpu, default=None),
            "syspolicyd_cpu_mean": round(sum(cpu) / len(cpu), 1) if cpu else None,
            "owner_cpu_max": max(owner, default=None),
            "owner_fds_max": max(owner_fds, default=None),
            "fixture_pids_seen": len(sampler.lf_pids[phase]),
        }
    return summary


def _group(rows: list) -> dict:
    grouped = collections.OrderedDict()
    for row in rows:
        grouped.setdefault(row["phase"], []).append(row)
    return grouped


def syspolicyd_log(start: float, end: float) -> dict:
    fmt = "%Y-%m-%d %H:%M:%S"
    out = subprocess.run(
        ["log", "show", "--style", "compact", "--info", "--start", time.strftime(fmt, time.localtime(start)),
         "--end", time.strftime(fmt, time.localtime(end)), "--predicate", 'process == "syspolicyd"'],
        capture_output=True, text=True, timeout=180,
    ).stdout
    (EVIDENCE / "syspolicyd.log").write_text(out)
    counts = collections.Counter()
    signatures = {"too_many_open_files": 0, "100024": 0, "failed_to_call_driver": 0, "missing_file": 0}
    for line in out.splitlines():
        message = re.sub(r"^\S+ \S+\s+\S+\s+\S+\s*", "", line)
        message = re.sub(r"\b0x[0-9a-f]+\b|\b\d+\b", "N", message)
        counts[message[:120]] += 1
        signatures["too_many_open_files"] += "Too many open files" in line
        signatures["100024"] += "100024" in line
        signatures["failed_to_call_driver"] += "failed to call driver" in line
        signatures["missing_file"] += "No such file" in line
    return {"lines": len(out.splitlines()), "signatures": signatures, "top": counts.most_common(5)}


def lf_log_by_phase() -> dict:
    path = HOME / "logs/lf.log"
    if not path.exists():
        return {}
    data = path.read_bytes()
    result = {}
    bounds = [m[2] for m in marks] + [len(data)]
    for index, (phase, _, offset) in enumerate(marks):
        lines = data[offset:bounds[index + 1]].decode(errors="replace").splitlines()
        counts = collections.Counter(re.sub(r"^\S+\s+", "", line)[:140] for line in lines)
        result[phase] = {"lines": len(lines), "top": counts.most_common(3)}
    return result


def wait_for_clear_host(limit: int = 900) -> float:
    """Launch nothing onto a pressured host: 15 straight seconds under the runner's line."""
    pid = subprocess.run(["pgrep", "-x", "syspolicyd"], capture_output=True, text=True).stdout.split()[0]
    started, clear = time.time(), 0
    while clear < PRESSURE_SECONDS // 5:
        if time.time() - started > limit:
            sys.exit("host-security pressure persisted; walkthrough not started")
        cpu = float(subprocess.run(["ps", "-o", "pcpu=", "-p", pid], capture_output=True, text=True).stdout)
        clear = clear + 1 if cpu < PRESSURE_CPU else 0
        time.sleep(5)
    return time.time() - started


def main():
    waited = wait_for_clear_host()
    results = {"checks": {}, "observations": {"waited_for_clear_host_seconds": round(waited)}}
    check = results["checks"]
    observed = results["observations"]
    for path in (DURABLE, RUNTIME, REOPEN_RUNTIME, PARKED):
        if path.exists():
            sys.exit(f"{path} already exists; inspect it before rerunning")
    for path in (HOME, REPO, PROVIDER_BIN, EVIDENCE, RUNTIME / "bin"):
        path.mkdir(parents=True)
    (HOME / "config.yaml").write_text(f"agent: claude\nsession:\n  end_detached_after: {GRACE}s\n")
    stand_in = PROVIDER_BIN / "claude"
    stand_in.write_text(STAND_IN)
    stand_in.chmod(0o755)
    (REPO / "AGENTS.md").write_text("Fixture repository.\n")
    for args in (["init", "--quiet", "-b", "fixture"], ["add", "."], ["commit", "--quiet", "-m", "fixture"], ["branch", "main"]):
        subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", *args],
                       cwd=REPO, check=True, env={"PATH": "/usr/bin:/bin", "HOME": str(HOME)})
    lf = RUNTIME / "bin/lf"
    shutil.copy2(BUILT, lf)
    copied_inode = lf.stat().st_ino
    built_inode = BUILT.stat().st_ino

    sampler = Sampler()
    sampler.start()
    sampler.set("baseline")
    time.sleep(15)
    first_start = time.time()

    term = None
    facts = {}
    try:
        sampler.set("launch")
        started = time.time()
        initialized = run_private(lf, "machine", "id", timeout=60)
        observed["first_exec_seconds"] = round(time.time() - started, 2)
        check["machine_initialized"] = initialized.returncode == 0
        term = Term(lf, name="launch")
        check["editor_ready"] = term.wait("EDITOR_READY", 45)
        if not check["editor_ready"]:
            raise RuntimeError("stand-in never reached its editor")
        time.sleep(1)
        facts = session_facts()
        results["ids"] = facts
        sampler.owner = facts["owner_pid"]
        observed["owner_command"] = command_of(facts["owner_pid"])[:160]

        sampler.set("typing")
        term.send("draft worlX")
        check["typed_draft_echoed"] = term.wait("EDITOR:draft worlX", 5)
        time.sleep(1)
        observed["native_input_after_typing"] = session_facts()["owner_native_input"]

        sampler.set("store-busy")
        lock = sqlite3.connect(HOME / "loopflow.db", isolation_level=None, timeout=5)
        lock.execute("BEGIN IMMEDIATE")
        term.send("Y")
        time.sleep(1.5)
        check["busy_input_waits_for_retention"] = not term.saw("EDITOR:draft worlXY")
        lock.execute("ROLLBACK")
        lock.close()
        check["busy_input_delivers_after_unlock"] = term.wait("EDITOR:draft worlXY", 5)
        term.send("\x7f")
        term.wait("EDITOR:draft worlX\r\n", 2)

        sampler.set("detach")
        term.close()
        term = None
        time.sleep(GRACE + 5)
        check["draft_owner_survives_detach_grace"] = alive(facts["owner_pid"])
        check["provider_survives_detach_grace"] = alive(facts["provider_pid"])

        sampler.set("rebuild")
        (REPO_ROOT / "rust/loopflow/src/bin/lf.rs").touch()
        env = {k: v for k, v in os.environ.items() if not k.startswith("LF_")}
        build = subprocess.run(["cargo", "build", "-p", "loopflow", "--bin", "lf"], cwd=REPO_ROOT, env=env,
                               stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=400)
        check["rebuild_succeeded"] = build.returncode == 0
        observed["rebuild_replaced_built_inode"] = BUILT.stat().st_ino != built_inode
        check["copied_runtime_unchanged"] = lf.stat().st_ino == copied_inode
        check["owner_survives_rebuild"] = alive(facts["owner_pid"])

        sampler.set("resume")
        term = Term(lf, "resume", facts["session"], name="resume")
        time.sleep(3)
        term.send("\x7fd")
        check["same_editor_after_resume"] = term.wait("EDITOR:draft world", 8)
        check["one_provider_after_resume"] = len(launches()) == 1
        time.sleep(1)

        remnants = EVIDENCE / "remnants"
        remnants.mkdir()
        parked_store = DURABLE / "parked-store"
        parked_store.mkdir()

        sampler.set("store-missing")
        for name in STORE_FILES:
            if (HOME / name).exists():
                os.rename(HOME / name, parked_store / name)
        time.sleep(2)
        term.send("a")
        observed["missing_store_input_echo"] = term.wait("EDITOR:draft worlda", 5)
        time.sleep(6)
        observed["missing_store_remnant"] = sorted(p.name for p in HOME.iterdir() if p.name.startswith("loopflow.db"))
        for name in STORE_FILES:
            if (HOME / name).exists():
                os.rename(HOME / name, remnants / name)
            if (parked_store / name).exists():
                os.rename(parked_store / name, HOME / name)
        time.sleep(2)

        sampler.set("root-moved")
        os.rename(DURABLE, PARKED)
        time.sleep(2)
        term.send("b")
        observed["moved_root_input_echo"] = term.wait("EDITOR:draft worldab", 5)
        time.sleep(6)
        observed["moved_root_recreated"] = DURABLE.exists()
        recreated = sorted(str(p.relative_to(DURABLE)) for p in DURABLE.rglob("*")) if DURABLE.exists() else []
        observed["moved_root_recreated_paths"] = recreated[:20]
        if DURABLE.exists():
            os.rename(DURABLE, PARKED / "evidence/remnants/recreated-root")
        os.rename(PARKED, DURABLE)

        sampler.set("store-restored")
        time.sleep(3)
        check["queued_input_delivered_once_after_restore"] = term.wait("EDITOR:draft worldab", 10)
        term.send("c")
        check["input_continues_after_restore"] = term.wait("EDITOR:draft worldabc", 8)
        observed["duplicate_a"] = term.saw("EDITOR:draft worldaa")
        observed["duplicate_b"] = term.saw("EDITOR:draft worldabb")
        check["owner_survives_store_loss"] = alive(facts["owner_pid"])
        check["one_provider_after_store_loss"] = len(launches()) == 1
        term.send("\r")
        deadline = time.time() + 10
        submitted = HOME / "submitted"
        while time.time() < deadline and not submitted.exists():
            time.sleep(0.1)
        check["draft_submitted_intact"] = submitted.exists() and submitted.read_text() == "draft worldabc\n"
        time.sleep(2)
        observed["native_input_after_submit"] = session_facts()["owner_native_input"]

        sampler.set("teardown")
        (RUNTIME / ".disposable-coding-demo").write_text("disposable runtime; evidence stored elsewhere")
        observed["holders_with_viewer"] = root_holders(RUNTIME)
        observed["provider_parent_is_owner"] = launches()[0]["ppid"] == facts["owner_pid"]
        term.close()
        term = None
        time.sleep(GRACE + 6)
        observed["edited_settled_owner_retained"] = alive(facts["owner_pid"])
        log = (HOME / "logs/lf.log").read_text()
        observed["retention_log_lines"] = log.count("retaining detached Session past its limit: native input")
        holders = root_holders(RUNTIME)
        observed["runtime_holders_detached"] = holders
        observed["provider_holds_runtime_root"] = str(facts["provider_pid"]) in holders
        started = time.time()
        term = Term(lf, "resume", facts["session"], name="teardown")
        time.sleep(2)
        term.send("\x04")
        deadline = time.time() + 30
        while time.time() < deadline and (alive(facts["owner_pid"]) or alive(facts["provider_pid"])):
            time.sleep(0.1)
        observed["provider_exit_teardown_seconds"] = round(time.time() - started, 1)
        term.close()
        term = None
        sampler.owner = None
        check["owner_ended_by_provider_exit"] = not alive(facts["owner_pid"])
        check["provider_not_left_behind"] = not alive(facts["provider_pid"])
        holders = root_holders(RUNTIME)
        check["runtime_root_unheld"] = not holders
        if not holders:
            shutil.rmtree(RUNTIME)
        check["runtime_root_removed"] = not RUNTIME.exists()
        check["durable_history_kept"] = (HOME / "loopflow.db").exists()

        sampler.set("reopen")
        (REOPEN_RUNTIME / "bin").mkdir(parents=True)
        reopen_lf = REOPEN_RUNTIME / "bin/lf"
        shutil.copy2(BUILT, reopen_lf)
        term = Term(reopen_lf, "resume", facts["session"], name="reopen")
        check["reopened_from_history"] = term.wait("EDITOR_RESUMED", 30)
        time.sleep(1)
        launched = launches()
        results["launches"] = [{"pid": l["pid"], "resume": "--resume" in l["argv"]} for l in launched]
        check["reopen_used_resume"] = len(launched) == 2 and "--resume" in launched[-1]["argv"]
        check["reopen_no_replay"] = submitted.read_text() == "draft worldabc\n"
        reopened = session_facts()
        results["reopened_ids"] = reopened
        check["same_session_reopened"] = reopened["session"] == facts["session"]
        term.send("plain\r")
        check["reopened_conversation_accepts_input"] = term.wait("EDITOR:\r\n", 8)
        time.sleep(1)
        check["plain_submission_settles"] = submitted.read_text() == "draft worldabc\nplain\n"
        started = time.time()
        term.close()
        term = None
        pid = reopened["owner_pid"]
        deadline = time.time() + 30
        while time.time() < deadline and pid and alive(pid):
            time.sleep(0.2)
        observed["plain_owner_self_end_seconds"] = round(time.time() - started, 1)
        check["plain_owner_ended_itself"] = not (pid and alive(pid))
        check["reopened_provider_ended"] = not (reopened["provider_pid"] and alive(reopened["provider_pid"]))
        holders = root_holders(REOPEN_RUNTIME)
        observed["reopen_runtime_holders"] = holders
        if not holders:
            shutil.rmtree(REOPEN_RUNTIME)
        check["reopen_runtime_removed"] = not REOPEN_RUNTIME.exists()

        sampler.set("after")
        time.sleep(15)
    except Exception as error:  # record and tear down; the receipt reports it
        results["error"] = repr(error)
    finally:
        if term:
            term.close()
        if PARKED.exists() and not DURABLE.exists():
            os.rename(PARKED, DURABLE)
        stop_verified(facts.get("provider_pid"), results)
        stop_verified(facts.get("owner_pid"), results)
        reopened_ids = results.get("reopened_ids", {})
        stop_verified(reopened_ids.get("provider_pid"), results)
        stop_verified(reopened_ids.get("owner_pid"), results)
        sampler.stopped.set()
        sampler.join(5)
        results["samples"] = summarize(sampler)
        results["lf_log"] = lf_log_by_phase()
        results["syspolicyd_log"] = syspolicyd_log(first_start - 15, time.time())
        results["phases"] = [(phase, round(at - first_start, 1)) for phase, at, _ in marks]
        (EVIDENCE / "results.json").write_text(json.dumps(results, indent=1, default=str))
        print(json.dumps(results, indent=1, default=str))


if __name__ == "__main__":
    main()
