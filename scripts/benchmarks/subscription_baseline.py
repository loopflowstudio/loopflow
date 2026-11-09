"""Measure actual server connections; writer, driver and server are separate processes."""

import argparse
import hashlib
import json
import os
import platform
import select
import signal
import socket
import subprocess
import tempfile
import time
from pathlib import Path


class Peer:
    def __init__(self, path: Path):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(60)
        self.socket.connect(str(path))
        self.buffer = b""
        self.bytes = 0

    def send(self, message: dict) -> None:
        self.socket.sendall(json.dumps(message).encode() + b"\n")

    def receive(self) -> dict:
        while b"\n" not in self.buffer:
            data = self.socket.recv(1024 * 1024)
            if not data:
                raise RuntimeError("socket closed")
            self.bytes += len(data)
            self.buffer += data
        line, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(line)

    def close(self) -> None:
        self.socket.close()


def _control(peer: Peer, action: str, **kwargs) -> dict:
    peer.send(dict(action=action, **kwargs))
    return peer.receive()


def _wait(peers: list[Peer], pending: set, accept, seconds: float = 60) -> list:
    deadline = time.monotonic() + seconds
    observed = []
    while pending:
        ready = [p for p in peers if b"\n" in p.buffer]
        if not ready:
            sockets, _, _ = select.select([p.socket for p in peers], [], [], 0.1)
            ready = [p for p in peers if p.socket in sockets]
        for peer in ready:
            message = peer.receive()
            if message["type"] == "error":
                raise RuntimeError(message)
            key = accept(message)
            if key in pending:
                pending.remove(key)
                observed.append((key, time.monotonic()))
        if time.monotonic() > deadline:
            raise TimeoutError(f"waiting for {pending}")
    return observed


def _drained(control: Peer) -> None:
    deadline = time.monotonic() + 60
    while True:
        s = _control(control, "snapshot")
        stages = s["stages"]
        builds = stages.get("runtime_build", {}).get("count", 0)
        drops = stages.get("runtime_drop", {}).get("count", 0)
        if builds == drops and s["opens"] == s["closes"]:
            return
        if time.monotonic() > deadline:
            raise TimeoutError(f"readers did not drain: {s}")
        time.sleep(0.005)


def _phase(control: Peer, results: dict, name: str, run):
    _control(control, "begin", name=name)
    began = time.monotonic()
    value = run()
    result = _control(control, "end", name=name)
    result["elapsed_ms"] = (time.monotonic() - began) * 1000
    results[name] = result
    return value


def _sample(
    binary: Path, output: Path, count: int, physical: bool, reverse: bool, profile: bool
) -> dict:
    peers = []
    control = None
    with tempfile.TemporaryDirectory(prefix="lf-subscription-", dir="/tmp") as directory:
        home = Path(directory)
        env = {k: v for k, v in os.environ.items() if not k.startswith("LF_")}
        env.update(HOME=directory, SUBSCRIPTION_HOME=directory, TOKIO_WORKER_THREADS="2")
        command = [
            str(binary),
            "--exact",
            "server::service::subscription_measurement::subscription_server",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]
        with output.with_suffix(".log").open("w") as log:
            process = subprocess.Popen(
                command,
                cwd=home,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                deadline = time.monotonic() + 120
                while not (home / "control.sock").exists() or not (home / "server.sock").exists():
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f"server setup failed; see {output.with_suffix('.log')}")
                    time.sleep(0.05)
                control = Peer(home / "control.sock")
                for _ in range(count if physical else 1):
                    peer = Peer(home / "server.sock")
                    peer.send(
                        dict(
                            type="hello",
                            revision=2,
                            min_revision=2,
                            client=dict(
                                kind="benchmark",
                                platform=platform.system(),
                                name="fixture",
                                version="1",
                            ),
                        )
                    )
                    assert peer.receive()["type"] == "welcome"
                    peers.append(peer)
                operation = dict(
                    kind="session.list",
                    directory=str(home / "repo"),
                    headless=True,
                    after=None,
                    limit=None,
                )
                phases = {}
                readings = []
                acknowledgements = []
                checkpoint_latencies = []
                serial = 0

                def cohort(size: int):
                    nonlocal serial
                    serial += 1
                    ids = [f"{serial}-{i}" for i in range(size)]
                    began = time.monotonic()
                    for i, id_ in enumerate(ids):
                        peers[i % len(peers)].send(
                            dict(type="subscribe", id=id_, operation=operation, select=None)
                        )
                    pending = {(id_, kind) for id_ in ids for kind in ("response", "reading")}

                    def accept(message):
                        if message["type"] == "reading":
                            frame = message["frame"]
                            assert frame["unavailable"] is None, frame
                            body = frame["body"]
                            assert len(body) == 127, (
                                f"expected 127 scoped Sessions, got {len(body)}"
                            )
                            normalized = sorted(
                                (row["id"], row["title"], row["state"]) for row in body
                            )
                            readings.append(
                                hashlib.sha256(json.dumps(normalized).encode()).hexdigest()
                            )
                            if not output.with_suffix(".reading.json").exists():
                                output.with_suffix(".reading.json").write_text(
                                    json.dumps(body, indent=2)
                                )
                        return message.get("id"), message["type"]

                    arrival = _wait(peers, pending, accept)
                    acknowledgements.extend(
                        {"kind": key[1], "ms": (at - began) * 1000} for key, at in arrival
                    )
                    began = time.monotonic()
                    for i, id_ in enumerate(ids):
                        peers[i % len(peers)].send(dict(type="checkpoint", id=id_, checkpoint=1))
                    arrival = _wait(
                        peers,
                        set(ids),
                        lambda m: (
                            m.get("id")
                            if m["type"] == "reading" and m["frame"]["answers"] == 1
                            else None
                        ),
                    )
                    checkpoint_latencies.extend((at - began) * 1000 for _, at in arrival)
                    return ids

                def cancel(ids):
                    for i, id_ in enumerate(ids):
                        peers[i % len(peers)].send(dict(type="cancel", id=id_))
                    # A one-shot answer passes the connection's cancel dispatch.
                    peers[0].send(
                        dict(
                            type="request",
                            id="barrier",
                            operation=dict(kind="config.catalog"),
                            select=None,
                        )
                    )
                    _wait(
                        peers,
                        {"barrier"},
                        lambda m: m.get("id") if m["type"] == "response" else None,
                    )
                    _drained(control)

                cold = _phase(control, phases, "cold_admission_checkpoint", lambda: cohort(1))
                _phase(control, phases, "cold_cleanup", lambda: cancel(cold))
                time.sleep(2.2)  # let the cold observer release its filesystem watch
                resources = {"warm": _control(control, "resources")}
                ids = _phase(control, phases, "admission_checkpoint", lambda: cohort(count))
                resources["cohort"] = _control(control, "resources")

                def window(active):
                    began = time.monotonic()
                    writes = []
                    frames = 0
                    next_write = began + 0.5
                    while time.monotonic() - began < 10:
                        if active and len(writes) < 10 and time.monotonic() >= next_write:
                            writes.append(_control(control, "write"))
                            next_write += 1
                        sockets, _, _ = select.select([p.socket for p in peers], [], [], 0.02)
                        for peer in peers:
                            if peer.socket in sockets or b"\n" in peer.buffer:
                                message = peer.receive()
                                assert message["type"] == "reading", message
                                assert message["frame"]["unavailable"] is None
                                frames += 1
                    return dict(writes=writes, frames=frames)

                windows = {}
                for active in [True, False] if reverse else [False, True]:
                    name = "active" if active else "quiet"
                    windows[name] = _phase(control, phases, name, lambda: window(active))
                changes = {}
                for action in ("title", "exit"):
                    began = time.monotonic()
                    _control(control, action)

                    def changed(message):
                        if message["type"] != "reading":
                            return None
                        body = message["frame"]["body"]
                        by_id = {row["id"]: row for row in body}
                        seen = (
                            by_id["session-001"]["title"] == "Changed by fixture"
                            if action == "title"
                            else by_id["session-000"]["state"] == "closed"
                        )
                        return message["id"] if seen else None

                    arrivals = _wait(peers, set(ids), changed)
                    changes[action] = [(at - began) * 1000 for _, at in arrivals]
                _phase(control, phases, "cleanup", lambda: cancel(ids))
                siblings = []

                def churn(cycles, size):
                    for i in range(cycles):
                        ids = cohort(size)
                        if i % 16 == 0:
                            began = time.monotonic()
                            peers[0].send(
                                dict(
                                    type="request",
                                    id="sibling",
                                    operation=dict(kind="config.catalog"),
                                    select=None,
                                )
                            )
                            _wait(
                                peers,
                                {"sibling"},
                                lambda m: m.get("id") if m["type"] == "response" else None,
                            )
                            siblings.append((time.monotonic() - began) * 1000)
                        cancel(ids)

                _phase(control, phases, "churn32", lambda: churn(32, 1))
                _phase(control, phases, "churn128", lambda: churn(128, 1))
                profiler = None
                if profile:
                    profiler = subprocess.Popen(
                        [
                            "/usr/bin/sample",
                            str(process.pid),
                            "5",
                            "1",
                            "-file",
                            str(output.with_suffix(".sample").resolve()),
                        ],
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL,
                    )
                try:
                    _phase(control, phases, "cohort_churn5", lambda: churn(5, count))
                finally:
                    if profiler:
                        profiler.wait(timeout=15)
                _phase(control, phases, "after_cleanup", lambda: time.sleep(2.2))
                final = _control(control, "snapshot")
                resources["drained"] = _control(control, "resources")
                assert phases["after_cleanup"]["statements"] == 0, phases["after_cleanup"]
                assert phases["after_cleanup"]["stages"].get("projection", {}).get("count", 0) == 0
                assert len(windows["active"]["writes"]) == 10
                assert final["opens"] == final["closes"]
                encoded_bytes = sum(peer.bytes for peer in peers)
                for peer in peers:
                    peer.close()
                peers.clear()
                control.send(dict(action="stop"))
                control.close()
                control = None
                assert process.wait(timeout=60) == 0
                stopped = json.loads((home / "final.json").read_text())
                result = dict(
                    count=count,
                    sockets=count if physical else 1,
                    phases=phases,
                    changes_ms=changes,
                    resources=resources,
                    encoded_bytes=encoded_bytes,
                    windows=windows,
                    admission_ms=acknowledgements,
                    checkpoint_ms=checkpoint_latencies,
                    sibling_ms=siblings,
                    normalized_readings=readings,
                    before_shutdown=final,
                    after_shutdown=stopped,
                    command=command,
                )
                output.write_text(json.dumps(result, indent=2) + "\n")
                return result
            finally:
                if not output.exists() and "phases" in locals():
                    output.with_suffix(".partial.json").write_text(json.dumps(phases, indent=2))
                for peer in peers:
                    peer.close()
                if control:
                    control.close()
                if process.poll() is None:
                    # Only this fixture's new process group; never installed LF.
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=30)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--counts", default="1,16,64")
    parser.add_argument("--multiplexed-only", action="store_true")
    parser.add_argument("--profile", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    sources = [
        Path(__file__),
        Path("rust/loopflow/src/server/subscription_measurement.rs"),
        Path("rust/loopflow/src/performance/subscription_measurement.rs"),
        Path("rust/loopflow/src/lf/commands/subscribe.rs"),
        Path("rust/loopflow/src/server/service.rs"),
    ]
    metadata = dict(
        binary=str(binary),
        sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        host=platform.platform(),
        cpu_count=os.cpu_count(),
        runtime_workers=2,
        blocking_workers=4,
        sources={str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
        rounds=args.rounds,
        profile=args.profile,
        operation="session.list",
        sessions=128,
        tasks=64,
        graphs=32,
        limits=(
            "No kernel wakeups, liveness probe or Git child counts. "
            "Child CPU is reaped children only."
        ),
    )
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    shapes = [
        (n, physical)
        for n in map(int, args.counts.split(","))
        for physical in ([False] if args.multiplexed_only or n == 1 else [False, True])
    ]
    assert all(n in (1, 16, 64) for n, _ in shapes)
    for repeat in range(args.rounds):
        for count, physical in shapes if repeat % 2 == 0 else list(reversed(shapes)):
            label = f"{repeat + 1}-{count}-{'physical' if physical else 'multiplexed'}"
            print(f"Starting {label}", flush=True)
            result = _sample(
                binary,
                args.output / f"{label}.json",
                count,
                physical,
                repeat % 2 == 1,
                args.profile,
            )
            p = result["phases"]["admission_checkpoint"]
            print(
                f"{label}: {p['user_ms'] + p['system_ms']:.2f} CPU ms, "
                f"{p['stages']['projection']['count']} projections",
                flush=True,
            )


if __name__ == "__main__":
    main()
