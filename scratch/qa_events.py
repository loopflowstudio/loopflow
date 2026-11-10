import json
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target/debug/lf"
BASE = Path(tempfile.mkdtemp(prefix="loo802-qa-"))
REPO = BASE / "repo"
REPO.mkdir()
ENV = {k: v for k, v in os.environ.items() if not k.startswith("LF_")}
ENV.update(HOME=str(BASE / "home"), LF_HOME=str(BASE / "machine"), LF_BIN=str(BIN), GIT_CONFIG_NOSYSTEM="1")
ENV.update(CODEX_HOME=str(BASE / "home/.codex"), CLAUDE_CONFIG_DIR=str(BASE / "home/.claude"))
Path(ENV["HOME"]).mkdir()
LOG = ROOT / "scratch/qa-transcript.jsonl"
LOG.write_text("")


def record(kind, value):
    with LOG.open("a") as f:
        f.write(json.dumps({"kind": kind, "value": value}) + "\n")


def run(args, binary=None, timeout=90):
    command = [str(binary or BIN), *args]
    result = subprocess.run(command, cwd=REPO, env=ENV, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=timeout)
    record("command", {"argv": command, "code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
    if result.returncode:
        raise RuntimeError(f"{args}: {result.stderr} {result.stdout}")
    return result.stdout


def cli(*args):
    return json.loads(run(list(args)))


def git(*args):
    return run(list(args), binary="git")


def stop_task_owners():
    processes = {}
    for line in subprocess.check_output(["ps", "-axo", "pid=,ppid=,command="], text=True).splitlines():
        fields = line.split(None, 2)
        if len(fields) == 3:
            processes[int(fields[0])] = (int(fields[1]), fields[2])
    owned = set()
    for pid, (_, command) in processes.items():
        if command.startswith(str(BIN) + " session serve"):
            cwd = subprocess.run(["lsof", "-a", "-p", str(pid), "-d", "cwd", "-Fn"], capture_output=True, text=True).stdout
            if str(BASE.resolve()) + "/" in cwd:
                owned.add(pid)
    while True:
        children = {pid for pid, (parent, _) in processes.items() if parent in owned}
        if children <= owned:
            break
        owned |= children
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        for pid in sorted(owned, reverse=True):
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        time.sleep(.5)
    record("cleanup_owners", {"processes": sorted(owned)})


class Client:
    def __init__(self, path):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.connect(path)
        self.buffer = b""
        self.frames = []
        self.send(json.loads((ROOT / "tests/fixtures/dto/server/hello.json").read_text()))
        self.until(lambda f: f["type"] == "welcome")

    def send(self, value):
        record("send", value)
        self.socket.sendall(json.dumps(value).encode() + b"\n")

    def recv(self, timeout=15):
        end = time.monotonic() + timeout
        while b"\n" not in self.buffer:
            if not select.select([self.socket], [], [], max(0, end-time.monotonic()))[0]:
                raise TimeoutError("no server frame")
            data = self.socket.recv(65536)
            if not data:
                raise EOFError("server closed")
            self.buffer += data
        line, self.buffer = self.buffer.split(b"\n", 1)
        frame = json.loads(line)
        self.frames.append(frame)
        record("receive", frame)
        return frame

    def until(self, predicate, timeout=30):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            frame = self.recv(end-time.monotonic())
            if predicate(frame):
                return frame
        raise TimeoutError("expected frame absent")

    def drain(self, seconds=0.8):
        end = time.monotonic() + seconds
        frames = []
        while time.monotonic() < end:
            try:
                frames.append(self.recv(end-time.monotonic()))
            except TimeoutError:
                break
        return frames

    def subscribe(self, id, operation, selection=None, resume=None):
        self.send(dict(type="subscribe", id=id, operation=operation, select=selection, resume=resume))
        return self.until(lambda f: f["type"] == "reading" and f["id"] == id)

    def close(self):
        self.socket.close()


def main():
    git("init", "-b", "main")
    git("config", "user.name", "Event QA")
    git("config", "user.email", "qa@example.invalid")
    git("init", "--bare", str(BASE / "origin.git"))
    git("remote", "add", "origin", str(BASE / "origin.git"))
    (REPO / ".lf/graphs").mkdir(parents=True)
    (REPO / ".lf/config.yaml").write_text("default_agent: codex\nplanning: {provider: git, personal: true}\ngraph: qa-normal\n")
    (REPO / ".lf/graphs/qa-normal.yaml").write_text("- cmd: help\n- cmd: help\n")
    git("add", ".")
    git("commit", "-m", "Prepare event walkthrough")
    git("push", "-u", "origin", "main")
    record("environment", {"base": str(BASE), "branch_binary": str(BIN)})
    # Ordinary released-client state before the branch first opens this Machine.
    prior = json.loads(run(["session", "resume", "--prepare", "--json"], binary="/Applications/Loopflow.app/Contents/MacOS/lf"))
    record("released_session", prior)
    task = cli("task", "create", "--title", "Recover a report", "--request-id", "qa-report", "--json")["task"]
    task_id = task["id"]
    record("task", task)
    sessions = cli("session", "list", "--json", "--all")
    assert prior["id"] in json.dumps(sessions)
    record("passed", "branch reads the Session prepared by the installed release")
    server_log = (ROOT / "scratch/qa-server.log").open("w")
    server = subprocess.Popen([str(BIN), "server", "run"], cwd=REPO, env=ENV, stdin=subprocess.DEVNULL, stdout=server_log, stderr=server_log, start_new_session=True)
    clients = []
    try:
        for _ in range(100):
            status = cli("server", "status")
            if status.get("ready"):
                break
            time.sleep(.1)
        else:
            raise RuntimeError(f"server not ready: {status}")
        path = status["socket"]
        c = Client(path)
        clients.append(c)
        operation = dict(kind="task.get", directory=str(REPO), issue=task_id)
        baseline = c.subscribe("full", operation)
        c.subscribe("title", operation, ["task.title"])
        assert not [f for f in c.drain() if f["type"] == "event"]
        run(["comment", task_id, "Keep the examples. λ\nSecond line."])
        for id in ["full", "title"]:
            if not any(f.get("id") == id and f["type"] == "event" for f in c.frames):
                c.until(lambda f: f.get("id") == id and f["type"] == "event")
        events = [f for f in c.frames if f["type"] == "event"]
        assert [f["seq"] for f in events] == [1, 2]
        assert all(f["event"]["comment"]["body"].startswith("Keep the examples. λ\nSecond line.") for f in events)
        thread = cli("task", "show", task_id, "--json")
        assert events[0]["event"]["comment"] in thread["comments"]["comments"]
        assert events[0]["event"]["steer"]["id"] == events[1]["event"]["steer"]["id"]
        record("passed", "comment payload, selection-independent delivery, shared sequence and occurrence identity")
        c.send(dict(type="cancel", id="title"))
        c.send(dict(type="request", id="bad", operation={"kind":"task.future"}, select=None))
        assert c.until(lambda f: f.get("id") == "bad")["code"] == "invalid"
        run(["comment", task_id, "After cancellation"])
        c.until(lambda f: f["type"] == "event" and f["event"]["comment"]["body"].startswith("After cancellation"))
        c.drain()
        assert len([f for f in c.frames if f["type"] == "event" and f["id"] == "title"]) == 1
        assert [f["seq"] for f in c.frames if f["type"] == "event"] == [1,2,3]
        record("passed", "cancelled subscription stops; unknown operation fails alone")
        run(["-b", "task", "run", task_id], timeout=180)
        c.until(lambda f: f["type"] == "event" and f["event"]["kind"] == "graph.step_finished")
        c.drain(2)
        steps = [f for f in c.frames if f["type"] == "event" and f["event"]["kind"] == "graph.step_finished"]
        assert len(steps) == 2, steps
        record("passed", "two real Graph command exits arrive as typed step events")
        c.close()
        clients.remove(c)
        run(["comment", task_id, "Written while disconnected"])
        c = Client(path)
        clients.append(c)
        fresh = c.subscribe("fresh", operation)
        assert "Written while disconnected" in json.dumps(fresh)
        assert not [f for f in c.drain() if f["type"] == "event"]
        run(["comment", task_id, "After reconnect"])
        event = c.until(lambda f: f["type"] == "event")
        assert event["seq"] == 1
        assert event["event"]["comment"]["body"].startswith("After reconnect")
        record("passed", "reconnect restores missed comments as state without replay; event sequence restarts at one")
        show = cli("task", "show", task_id, "--json")
        assert "After reconnect" in json.dumps(show)
        record("passed", "CLI observes same final comments as socket")
        c2 = Client(path)
        clients.append(c2)
        plan = c2.subscribe("plans", dict(kind="plan.list", directory=str(REPO), wave=None, task=None, all=True))
        dashboard = c2.subscribe("dashboard", dict(kind="desktop.view.list", directory=str(REPO)))
        assert not [f for f in c2.drain() if f["type"] == "event"]
        another = cli("task", "create", "--title", "Burst comments", "--json")["task"]["id"]
        cli("desktop", "dashboard", "query", "Event walkthrough", f"task:{another}")
        for watch in ["plans", "dashboard"]:
            if not any(f["type"] == "reading" and f["id"] == watch and another in json.dumps(f) for f in c2.frames):
                c2.until(lambda f: f["type"] == "reading" and f["id"] == watch and another in json.dumps(f))
        for i in range(8):
            c2.send(dict(type="request", id=f"burst-{i}", operation=dict(kind="task.steer", directory=str(REPO), issue=another, message=f"Burst comment {i}"), select=None))
        while len([f for f in c2.frames if f["type"] in ["response", "error"] and f.get("id", "").startswith("burst-")]) < 8:
            c2.recv()
        refused = [f for f in c2.frames if f["type"] == "error" and f.get("id", "").startswith("burst-")]
        for refusal in refused:
            assert refusal["code"] == "busy" and refusal["data"].get("max")
            i = int(refusal["id"].split("-")[1])
            for attempt in range(30):
                c2.drain(.1)
                c2.send(dict(type="request", id=f"retry-{i}", operation=dict(kind="task.steer", directory=str(REPO), issue=another, message=f"Burst comment {i}"), select=None))
                result = c2.until(lambda f: f.get("id") == f"retry-{i}")
                if result["type"] == "response":
                    break
                assert result["code"] == "busy"
            else:
                raise RuntimeError("admission never recovered")
        while len([f for f in c2.frames if f["type"] == "event"]) < 16:
            c2.recv()
        c2.drain()
        burst = [f for f in c2.frames if f["type"] == "event"]
        assert [f["seq"] for f in burst] == list(range(1,17))
        for watch in ["plans", "dashboard"]:
            received = [f["event"]["comment"]["body"].split("\n")[0] for f in burst if f["id"] == watch]
            assert sorted(received) == [f"Burst comment {i}" for i in range(8)]
        record("passed", f"plan and dashboard admit a new Task; eight concurrent steers ({len(refused)} admission refusals retried) yield sixteen uncoalesced events in connection order")
        for _ in range(30):
            c2.drain(.1)
            c2.send(dict(type="request", id="empty", operation=dict(kind="task.steer", directory=str(REPO), issue=another, message="   "), select=None))
            invalid = c2.until(lambda f: f.get("id") == "empty")
            if invalid.get("code") != "busy":
                break
        assert invalid["type"] == "error" and "empty" in invalid["message"]
        assert len(cli("task", "show", another, "--json")["comments"]["comments"]) == 8
        record("passed", "empty comment rejected without changing the eight saved comments")
        for client in clients:
            client.close()
        clients.clear()
        os.killpg(server.pid, signal.SIGTERM)
        server.wait(timeout=10)
        server = subprocess.Popen([str(BIN), "server", "run"], cwd=REPO, env=ENV, stdin=subprocess.DEVNULL, stdout=server_log, stderr=server_log, start_new_session=True)
        for _ in range(100):
            replacement = cli("server", "status")
            if replacement.get("ready"):
                break
            time.sleep(.1)
        assert replacement["instance"] != status["instance"]
        c = Client(replacement["socket"])
        clients.append(c)
        restored = c.subscribe("restart", operation)
        assert "After reconnect" in json.dumps(restored)
        assert not [f for f in c.drain() if f["type"] == "event"]
        record("passed", "server restart returns persisted state without replaying old steers or Graph exits")
        record("probe_parameters", {"socket": path,"directory":str(REPO),"task_id": task_id})
        if (ROOT / "scratch/qa_swift_probe").exists():
            encoded = run(["encode", str(REPO), task_id], binary=ROOT / "scratch/qa_swift_probe")
            wire = socket.socket(socket.AF_UNIX)
            wire.connect(path)
            wire.sendall(encoded.encode())
            wire.settimeout(5)
            with wire.makefile("rb") as reader:
                frames = [json.loads(reader.readline()) for _ in range(4)]
            wire.close()
            assert {f["type"] for f in frames} == {"welcome", "response", "reading"}
            for frame in frames:
                record("receive", frame)
            run(["decode", str(LOG)], binary=ROOT / "scratch/qa_swift_probe")
            record("passed", "Swift production wire types encode requests and decode live Task events")
    finally:
        for client in clients:
            client.close()
        os.killpg(server.pid, signal.SIGTERM)
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(server.pid, signal.SIGKILL)
            server.wait()
        server_log.close()
        record("cleanup", {"server_reaped": server.returncode, "private_directory": str(BASE)})
        stop_task_owners()


if __name__ == "__main__":
    main()
