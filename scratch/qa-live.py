"""Drive the branch server through its public Unix-socket protocol."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import time


REPO = Path(__file__).resolve().parents[1]
BIN = REPO / "target/debug/lf"
TRANSCRIPT = []


def note(scenario, observed):
    TRANSCRIPT.append((scenario, observed))
    print(f"{scenario}: {observed}", flush=True)


class Client:
    def __init__(self, endpoint):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(30)
        self.socket.connect(str(endpoint))
        self.stream = self.socket.makefile("rwb", buffering=0)
        self.pending = []
        self.counter = 0
        self.send(type="hello", revision=2, min_revision=2,
                  client=dict(kind="cli", platform="macos", name="LOO-805 QA", version="0.15.2"))
        self.welcome = self.receive(lambda m: m["type"] in ("welcome", "error"))
        assert self.welcome["type"] == "welcome", self.welcome

    def send(self, **message):
        self.stream.write(json.dumps(message).encode() + b"\n")

    def receive(self, predicate):
        for i, message in enumerate(self.pending):
            if predicate(message):
                return self.pending.pop(i)
        while True:
            line = self.stream.readline()
            if not line:
                raise RuntimeError("Server closed the connection")
            message = json.loads(line)
            if predicate(message):
                return message
            self.pending.append(message)

    def request(self, operation, error=False):
        self.counter += 1
        request_id = f"r{self.counter}"
        started = time.monotonic()
        self.send(type="request", id=request_id, operation=operation, select=None)
        response = self.receive(lambda m: m.get("id") == request_id)
        elapsed = time.monotonic() - started
        assert response["type"] == ("error" if error else "response"), response
        return (response if error else response["result"]), elapsed

    def subscribe(self, identifier, operation):
        self.send(type="subscribe", id=identifier, operation=operation, select=None)
        response = self.receive(lambda m: m.get("id") == identifier)
        assert response["type"] == "response", response
        reading = self.receive(lambda m: m.get("id") == identifier and m["type"] == "reading")
        assert reading["frame"]["unavailable"] is None, reading
        return reading["frame"]["body"]

    def close(self):
        self.stream.close()
        self.socket.close()


def run():
    original = (REPO / "rust/loopflow/src/server/README.md").read_bytes()
    probe = REPO / "qa-loo805-live.txt"
    bulk = REPO / "qa-loo805-live-files"
    assert not probe.exists() and not bulk.exists()
    server = None
    clients = []
    endpoint = None
    with tempfile.TemporaryDirectory(prefix="loo805-qa-", dir="/private/tmp") as temporary:
        root = Path(temporary)
        state = root / "state"
        state.mkdir(mode=0o700)
        account = root / "account"
        account.mkdir()
        env = {k: v for k, v in os.environ.items() if not k.startswith("LF_")}
        env.update(LF_HOME=str(state), HOME=str(account))

        def cli(*args):
            return subprocess.run([str(BIN), *args], cwd=REPO, env=env,
                                  stdin=subprocess.DEVNULL, capture_output=True, text=True, check=True).stdout

        def git(*args):
            return subprocess.run(["git", *args], cwd=REPO, stdin=subprocess.DEVNULL,
                                  capture_output=True, text=True, check=True).stdout

        digest = hashlib.sha256(os.fsencode(state.resolve())).hexdigest()[:16]
        endpoint = Path(f"/private/tmp/lf-{os.geteuid()}-{digest}/machine.sock")

        def start():
            process = subprocess.Popen([str(BIN), "server", "run"], cwd=REPO, env=env,
                                       stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                assert process.poll() is None, "Server exited before readiness"
                if endpoint.exists():
                    try:
                        client = Client(endpoint)
                        clients.append(client)
                        return process, client
                    except ConnectionRefusedError:
                        pass
                time.sleep(0.05)
            process.terminate()
            process.wait(timeout=10)
            raise RuntimeError("Server readiness timed out")

        with (root / "server.log").open("w") as log:
            try:
                server, client = start()
                assert client.welcome["machine"] is None
                note("Fresh state", "Branch server welcomed protocol revision 2 with no initialized Machine; status ready=" + str(json.loads(cli("server", "status"))["ready"]))
                client.close()
                clients.remove(client)
                server.terminate()
                server.wait(timeout=10)
                server = None

                # An unchanged SQLite backup supplies existing Task data; no fixture rows or setters.
                source = Path(os.environ.get("LF_HOME", str(Path.home() / ".lf"))) / "loopflow.db"
                with sqlite3.connect(f"file:{source}?mode=ro", uri=True) as src:
                    with sqlite3.connect(state / "loopflow.db") as dst:
                        src.backup(dst)
                server, client = start()
                note("Returning state", "Restarted with a consistent backup of the existing Machine database; original store untouched.")
                task = dict(kind="task.get", directory=str(REPO), issue="LOO-805")
                changes = dict(kind="task.changes", issue="LOO-805", base="parent", head=None)
                first = client.subscribe("pane1", task)
                before, elapsed = client.request(changes)
                assert not any(f["path"] == probe.name for f in before["files"])
                note("Open Task", f"Subscribed to LOO-805 and read {len(before['files'])} existing changed files on the same connection ({elapsed:.3f}s).")
                head = git("rev-parse", "HEAD").strip()
                empty, _ = client.request(dict(changes, base=head, head=head))
                assert empty["files"] == []
                note("Empty comparison", "HEAD against HEAD returned files=[].")

                probe.write_text("first content\n")
                readme = REPO / "rust/loopflow/src/server/README.md"
                readme.write_bytes(original + b"\nLOO-805 live QA marker\n")
                after, elapsed = client.request(changes)
                files = {f["path"]: f for f in after["files"]}
                assert files[probe.name]["untracked"]
                assert files["rust/loopflow/src/server/README.md"]["unstaged"]
                note("Edit while subscribed", f"Immediate task.changes returned the new untracked file and the unstaged README edit ({elapsed:.3f}s).")
                diff, _ = client.request(dict(kind="task.diff", issue="LOO-805", path="rust/loopflow/src/server/README.md", base="parent", draft=None, head=None))
                assert "LOO-805 live QA marker" in json.dumps(diff)
                note("Read patch", "task.diff included +LOO-805 live QA marker on the existing connection.")
                second = client.subscribe("pane2", task)
                assert second["row"]["condition"]["local_progress"]["dirty"]
                git("add", "--", probe.name)
                staged, elapsed = client.request(changes)
                item = next(f for f in staged["files"] if f["path"] == probe.name)
                assert item["staged"] and not item["untracked"], item
                note("Two subscriptions and stage", f"Both Task subscriptions active; task.changes changed the file from untracked to staged immediately ({elapsed:.3f}s).")
                probe.write_text("second content\n")
                changed, _ = client.request(changes)
                item = next(f for f in changed["files"] if f["path"] == probe.name)
                assert item["staged"] and item["unstaged"], item
                content, _ = client.request(dict(kind="task.file", issue="LOO-805", path=probe.name))
                assert "second content" in json.dumps(content)
                note("Edit staged file", "Returned staged=true and unstaged=true; task.file returned second content.")

                failed, _ = client.request(dict(changes, base="qa-missing-revision-805"), error=True)
                healthy, _ = client.request(changes)
                assert any(f["path"] == probe.name for f in healthy["files"])
                note("Bad input then retry", f"Unknown base returned {failed['code']}: {failed['message']}; the next valid request succeeded on the same connection.")

                bulk.mkdir()
                for i in range(200):
                    (bulk / f"file-{i:03}.txt").write_text(f"file {i}\n")
                many, elapsed = client.request(changes)
                count = sum(f["path"].startswith(bulk.name + "/") for f in many["files"])
                assert count == 200, count
                note("Larger change set", f"All 200 new files appeared in one request while both subscriptions remained active ({elapsed:.3f}s).")
                client.send(type="checkpoint", id="pane2", checkpoint=805)
                reading = client.receive(lambda m: m.get("id") == "pane2" and m.get("frame", {}).get("answers") == 805)
                assert reading["frame"]["unavailable"] is None
                note("Subscription refresh", "Checkpoint 805 returned an available Task reading with dirty=true: " + str(reading["frame"]["body"]["row"]["condition"]["local_progress"]["dirty"]))

                client.close()
                clients.remove(client)
                client = Client(endpoint)
                clients.append(client)
                client.subscribe("resumed", task)
                resumed, _ = client.request(changes)
                assert sum(f["path"].startswith(bulk.name + "/") for f in resumed["files"]) == 200
                note("Disconnect and reconnect", "Closed a connection with two subscriptions; a new connection received a complete Task reading and all 200 files.")

                git("reset", "--", probe.name)
                probe.unlink()
                shutil.rmtree(bulk)
                readme.write_bytes(original)
                clean, _ = client.request(changes)
                assert not any(f["path"] == probe.name or f["path"].startswith(bulk.name + "/") for f in clean["files"])
                assert not next(f for f in clean["files"] if f["path"] == "rust/loopflow/src/server/README.md")["unstaged"]
                note("Undo edits", "Same-connection task.changes removed all QA files and cleared README's unstaged flag; original README bytes restored.")
                client.close()
                clients.remove(client)
                status = json.loads(cli("server", "status"))
                assert status["clients"] == [], status["clients"]
                note("Connection cleanup", "Server status reports clients=[] after disconnect.")
            finally:
                for client in clients:
                    client.close()
                if server is not None:
                    server.terminate()
                    server.wait(timeout=10)
                    note("Server cleanup", "QA server stopped and reaped.")
                if probe.exists():
                    git("reset", "--", probe.name)
                    probe.unlink()
                if bulk.exists():
                    shutil.rmtree(bulk)
                (REPO / "rust/loopflow/src/server/README.md").write_bytes(original)
                if endpoint.parent.exists():
                    shutil.rmtree(endpoint.parent)
                (REPO / "scratch/qa-transcript.json").write_text(json.dumps(TRANSCRIPT, indent=2) + "\n")


if __name__ == "__main__":
    run()
