"""Measure `lf server run` CPU, threads, forks, and store opens across three phases."""

from __future__ import annotations

import argparse
import json
import os
import platform
import select
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

BUDGETS_PATH = Path(__file__).resolve().parents[1] / "performance-budgets.json"


class ServerPeer:
    def __init__(self, path: Path) -> None:
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(30)
        self.socket.connect(str(path))
        self.buffer = b""

    def send(self, message: dict) -> None:
        self.socket.sendall(json.dumps(message).encode() + b"\n")

    def receive(self) -> dict:
        while b"\n" not in self.buffer:
            data = self.socket.recv(1024 * 1024)
            if not data:
                raise RuntimeError("server socket closed")
            self.buffer += data
        line, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(line)

    def drain_nonblocking(self) -> int:
        frames = 0
        while True:
            if b"\n" not in self.buffer:
                ready, _, _ = select.select([self.socket], [], [], 0.0)
                if not ready:
                    break
                data = self.socket.recv(1024 * 1024)
                if not data:
                    raise RuntimeError("server socket closed")
                self.buffer += data
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                message = json.loads(line)
                if message.get("type") == "reading":
                    frames += 1
        return frames

    def diagnostics(self, req_id: str) -> dict:
        self.send(
            {
                "type": "request",
                "id": req_id,
                "operation": {"kind": "server.diagnostics"},
                "select": None,
                "sender": None,
                "request_id": None,
            }
        )
        while True:
            message = self.receive()
            if message.get("type") == "response" and message.get("id") == req_id:
                return message["result"]
            if message.get("type") == "error" and message.get("id") == req_id:
                raise RuntimeError(f"server.diagnostics failed: {message}")

    def close(self) -> None:
        self.socket.close()


def _clean_env(home: Path) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if not k.startswith("LF_")}
    env["HOME"] = str(home)
    env["LF_HOME"] = str(home / ".lf")
    env["GIT_CONFIG_GLOBAL"] = str(home / ".gitconfig")
    return env


def _parse_cputime(text: str) -> float:
    """Parse `ps -o cputime=` output (`[DD-]HH:MM:SS[.cc]` or `MM:SS.cc`) to seconds."""
    text = text.strip()
    days = 0
    if "-" in text:
        day_part, text = text.split("-", 1)
        days = int(day_part)
    parts = text.split(":")
    if len(parts) == 2:
        minutes, seconds = int(parts[0]), float(parts[1])
        hours = 0
    elif len(parts) == 3:
        hours, minutes, seconds = int(parts[0]), int(parts[1]), float(parts[2])
    else:
        raise ValueError(f"unexpected cputime format: {text!r}")
    return days * 86400 + hours * 3600 + minutes * 60 + seconds


def _ps_cputime_seconds(pid: int) -> float:
    output = subprocess.check_output(
        ["ps", "-o", "cputime=", "-p", str(pid)],
        text=True,
    )
    return _parse_cputime(output)


def _ps_thread_count(pid: int) -> int:
    if platform.system() == "Darwin":
        output = subprocess.check_output(["ps", "-M", "-p", str(pid)], text=True)
        return max(0, len(output.strip().splitlines()) - 1)
    task_dir = Path(f"/proc/{pid}/task")
    return sum(1 for _ in task_dir.iterdir()) if task_dir.exists() else 0


def _seed_fixture(binary: Path, home: Path, env: dict[str, str]) -> tuple[Path, str, Path]:
    (home / ".gitconfig").write_text(
        "[user]\n\tname = Benchmark\n\temail = bench@loopflow.test\n[init]\n\tdefaultBranch = main\n"
    )
    lf_home = home / ".lf"
    lf_home.mkdir(parents=True, exist_ok=True)
    repos_root = home / "repos"
    repos_root.mkdir()

    repos: list[Path] = []
    for i in range(12):
        repo = repos_root / f"repo-{i:02d}"
        repo.mkdir()
        subprocess.run(["git", "init", "--quiet"], cwd=repo, env=env, check=True)
        (repo / "README.md").write_text(f"# repo {i}\n")
        (repo / "tracked.txt").write_text("initial\n")
        subprocess.run(["git", "add", "."], cwd=repo, env=env, check=True)
        subprocess.run(
            ["git", "commit", "--quiet", "-m", "init"],
            cwd=repo,
            env=env,
            check=True,
        )
        repos.append(repo.resolve())

    # Initialize the 113-table store and seed 13 Tasks with checkouts across the 12 repos.
    created_tasks: list[tuple[str, Path]] = []
    for i in range(13):
        repo = repos[i % len(repos)]
        out = subprocess.check_output(
            [str(binary), "task", "create", f"Bench task {i}", "--json"],
            cwd=repo,
            env=env,
            stdin=subprocess.DEVNULL,
            text=True,
        )
        created = json.loads(out)
        created_tasks.append((created["task"]["id"], repo))

    first_task_id = created_tasks[0][0]

    # Attach checkouts for all 13 Tasks and seed 64 Sessions directly in SQLite.
    db_path = lf_home / "loopflow.db"
    conn = sqlite3.connect(db_path)
    try:
        now = int(time.time())
        for idx, (task_id, repo) in enumerate(created_tasks):
            conn.execute(
                "INSERT OR IGNORE INTO task_checkouts "
                "(task_id, worktree, branch) "
                "VALUES (?, ?, ?)",
                (task_id, str(repo), f"bench-{idx}"),
            )
        for i in range(64):
            repo_path = str(repos[i % len(repos)])
            session_id = f"session_{i:032x}"
            conn.execute(
                "INSERT OR IGNORE INTO agent_sessions "
                "(id, repo, cwd, created_at, title) "
                "VALUES (?, ?, ?, ?, ?)",
                (session_id, repo_path, repo_path, now, f"Bench session {i}"),
            )
        conn.commit()
    finally:
        conn.close()

    return repos[0], first_task_id, db_path


def _wait_ready(binary: Path, env: dict[str, str], server: subprocess.Popen) -> Path:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if server.poll() is not None:
            raise RuntimeError(f"lf server run exited early with {server.returncode}")
        proc = subprocess.run(
            [str(binary), "server", "status"],
            env=env,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
        )
        if proc.returncode == 0 and proc.stdout.strip():
            status = json.loads(proc.stdout)
            if status.get("ready"):
                return Path(status["socket"])
        time.sleep(0.1)
    raise TimeoutError("lf server run did not become ready within 30s")


def _writer_loop(db_path: Path, stop_file: Path, rate_hz: float) -> None:
    conn = sqlite3.connect(db_path, timeout=15.0)
    interval = 1.0 / rate_hz
    seq = 0
    try:
        while not stop_file.exists():
            t0 = time.monotonic()
            seq += 1
            conn.execute(
                "UPDATE agent_sessions SET title = ? "
                "WHERE id = 'session_00000000000000000000000000000000'",
                (f"Bench write {seq}",),
            )
            conn.commit()
            remaining = interval - (time.monotonic() - t0)
            if remaining > 0:
                time.sleep(remaining)
    finally:
        conn.close()


def _measure_phase(
    name: str,
    pid: int,
    peer: ServerPeer,
    seconds: float,
    on_tick,
) -> dict:
    before_diag = peer.diagnostics(f"{name}-before")
    before_cpu = _ps_cputime_seconds(pid)
    t0 = time.monotonic()
    deadline = t0 + seconds
    max_threads = max(before_diag["threads"], _ps_thread_count(pid))
    frames = 0
    tick = 0
    while True:
        now = time.monotonic()
        if now >= deadline:
            break
        tick += 1
        on_tick(tick, now - t0)
        frames += peer.drain_nonblocking()
        if tick % 5 == 0:
            max_threads = max(max_threads, _ps_thread_count(pid))
        sleep_for = min(1.0, max(0.0, deadline - time.monotonic()))
        if sleep_for > 0:
            time.sleep(sleep_for)

    elapsed = time.monotonic() - t0
    after_cpu = _ps_cputime_seconds(pid)
    after_diag = peer.diagnostics(f"{name}-after")
    max_threads = max(max_threads, after_diag["threads"], _ps_thread_count(pid))
    cpu_percent = ((after_cpu - before_cpu) / elapsed) * 100.0 if elapsed > 0 else 0.0
    spawns = after_diag["spawns"] - before_diag["spawns"]
    store_opens = after_diag["store_opens"] - before_diag["store_opens"]
    return {
        "elapsed_s": round(elapsed, 2),
        "cpu_percent": round(cpu_percent, 2),
        "threads": max_threads,
        "spawns": spawns,
        "store_opens": store_opens,
        "held_facts": after_diag["held_facts"],
        "frames": frames,
    }


def _check_budgets(results: dict, seconds: float) -> list[str]:
    if not BUDGETS_PATH.exists():
        return []
    budgets = json.loads(BUDGETS_PATH.read_text()).get("server_idle")
    if not budgets:
        return []
    violations: list[str] = []
    for phase, limits in budgets.items():
        if phase not in results:
            continue
        r = results[phase]
        if r["cpu_percent"] > limits["max_cpu_percent"]:
            violations.append(
                f"{phase}: cpu_percent {r['cpu_percent']}% > {limits['max_cpu_percent']}%"
            )
        if r["threads"] >= limits["max_threads"]:
            violations.append(f"{phase}: threads {r['threads']} >= {limits['max_threads']}")
        if "max_spawns" in limits and r["spawns"] > limits["max_spawns"]:
            violations.append(f"{phase}: spawns {r['spawns']} > {limits['max_spawns']}")
        if "max_spawns_per_3s" in limits:
            allowed = int((seconds / 3.0) * limits["max_spawns_per_3s"]) + 1
            if r["spawns"] > allowed:
                violations.append(f"{phase}: spawns {r['spawns']} > {allowed}")
        if r["store_opens"] > limits["max_store_opens"]:
            violations.append(
                f"{phase}: store_opens {r['store_opens']} > {limits['max_store_opens']}"
            )
    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="Path to lf binary")
    parser.add_argument("--seconds", type=float, default=60.0, help="Seconds per phase")
    parser.add_argument("--no-fail", action="store_true", help="Exit 0 even past budgets")
    parser.add_argument("--json-out", type=Path, help="Write JSON results to this file")
    parser.add_argument("--_writer-db", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--_writer-stop", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()

    if args._writer_db and args._writer_stop:
        _writer_loop(args._writer_db, args._writer_stop, 20.0)
        return 0

    binary = args.binary.resolve()
    if not binary.exists():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="lf-idle-", dir="/tmp") as directory:
        home = Path(directory)
        env = _clean_env(home)
        repo0, task_id, db_path = _seed_fixture(binary, home, env)
        log_path = home / "server.log"

        with log_path.open("w") as log:
            server = subprocess.Popen(
                [str(binary), "server", "run"],
                cwd=home,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
            peer: ServerPeer | None = None
            writer: subprocess.Popen | None = None
            try:
                sock_path = _wait_ready(binary, env, server)
                peer = ServerPeer(sock_path)
                peer.send(
                    {
                        "type": "hello",
                        "revision": 11,
                        "min_revision": 10,
                        "client": {
                            "kind": "benchmark",
                            "platform": platform.system().lower(),
                            "name": "server_idle",
                            "version": "1",
                        },
                    }
                )
                welcome = peer.receive()
                assert welcome["type"] == "welcome", welcome

                # Open one board subscription and one Task-detail subscription.
                peer.send(
                    {
                        "type": "subscribe",
                        "id": "board-1",
                        "operation": {
                            "kind": "plan.list",
                            "directory": str(repo0),
                            "wave": None,
                            "task": None,
                            "all": True,
                        },
                        "select": None,
                        "resume": None,
                        "sender": None,
                    }
                )
                peer.send(
                    {
                        "type": "subscribe",
                        "id": "task-1",
                        "operation": {
                            "kind": "task.get",
                            "directory": str(repo0),
                            "issue": task_id,
                        },
                        "select": None,
                        "resume": None,
                        "sender": None,
                    }
                )
                needed = {("board-1", "reading"), ("task-1", "reading")}
                deadline = time.monotonic() + 30
                while needed and time.monotonic() < deadline:
                    msg = peer.receive()
                    if msg.get("type") == "error":
                        raise RuntimeError(f"subscription error: {msg}")
                    needed.discard((msg.get("id"), msg.get("type")))
                if needed:
                    raise TimeoutError(f"initial readings missing: {needed}")

                # Let initial startup settle before sampling.
                time.sleep(2.0)
                peer.drain_nonblocking()

                results: dict[str, dict] = {}
                results["quiet"] = _measure_phase(
                    "quiet", server.pid, peer, args.seconds, lambda _t, _e: None
                )

                tracked = repo0 / "tracked.txt"
                results["file_edits"] = _measure_phase(
                    "file_edits",
                    server.pid,
                    peer,
                    args.seconds,
                    lambda tick, _e: tracked.write_text(f"edit {tick}\n"),
                )

                # Let the final throttled Git refresh from `file_edits` land
                # before measuring `db_writes`.
                time.sleep(3.5)
                peer.drain_nonblocking()

                stop_file = home / "stop-writer"
                writer = subprocess.Popen(
                    [
                        sys.executable,
                        str(Path(__file__).resolve()),
                        str(binary),
                        "--_writer-db",
                        str(db_path),
                        "--_writer-stop",
                        str(stop_file),
                    ],
                    stdin=subprocess.DEVNULL,
                )
                try:
                    results["db_writes"] = _measure_phase(
                        "db_writes", server.pid, peer, args.seconds, lambda _t, _e: None
                    )
                finally:
                    stop_file.write_text("stop")
                    writer.wait(timeout=10)
                    writer = None
            finally:
                if peer is not None:
                    peer.close()
                if writer is not None and writer.poll() is None:
                    writer.terminate()
                    writer.wait(timeout=5)
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)

    print(f"{'Phase':<14} {'CPU%':>7} {'Threads':>8} {'Spawns':>8} {'Opens':>7} {'Held':>6} {'Frames':>7}")
    for phase, r in results.items():
        print(
            f"{phase:<14} {r['cpu_percent']:>6.2f}% {r['threads']:>8} "
            f"{r['spawns']:>8} {r['store_opens']:>7} {r['held_facts']:>6} {r['frames']:>7}"
        )

    if args.json_out:
        args.json_out.write_text(json.dumps(results, indent=2) + "\n")

    violations = _check_budgets(results, args.seconds)
    if violations:
        print("\nBudget violations:", file=sys.stderr)
        for v in violations:
            print(f"  - {v}", file=sys.stderr)
        return 0 if args.no_fail else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
