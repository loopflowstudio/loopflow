# /// script
# requires-python = ">=3.10"
# dependencies = ["cryptography>=44"]
# ///
"""Prove Chapter CLI adoption/default launch in a disposable Linux OS account.

Uses synthetic Linear HTTPS and the existing scripted Codex socket transport.
No real credentials, provider service, installation, or production endpoint override.
"""

import argparse
import base64
import copy
import hashlib
import json
import os
import pwd
import shlex
import shutil
import signal
import sqlite3
import subprocess
import sys
import threading
import time
import uuid
from http.server import ThreadingHTTPServer
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from linear_oauth import _encrypt, _tls_context
from task_deletion import Handler as ProxyHandler

FLOW = "chapter-proof"
MARKER = "CHAPTER_PROJECT_DEFAULT_REACHED"


def _page(nodes: list[dict]) -> dict:
    return {"nodes": nodes, "pageInfo": {"hasNextPage": False, "endCursor": None}}


class ChapterHandler(ProxyHandler):
    # Reuse task_deletion's CONNECT/TLS transport; all GraphQL state is local.
    def do_POST(self) -> None:
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        query, variables = request["query"], request.get("variables", {})
        try:
            with self.server.state_lock:
                data = _graphql(self.server.state, query, variables)
            payload = {"data": data}
        except (AssertionError, KeyError) as error:
            self.server.errors.append(str(error))
            payload = {"errors": [{"message": str(error)}]}
        body = json.dumps(payload).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def _graphql(state: dict, query: str, variables: dict) -> dict:
    operation = query.split("{", 1)[0].split("(", 1)[0].split()[-1]
    state["operations"].append(operation)
    projects, issue = state["projects"], state["issue"]
    identity = variables.get("id")
    if operation == "ListTeams":
        return {
            "teams": _page(
                [
                    {
                        "id": "team-1",
                        "name": "Fixture",
                        "key": "FIX",
                        "description": "<!-- loopflow-repository: loopflowstudio/fixture -->",
                    }
                ]
            )
        }
    if operation == "ListInitiatives":
        return {
            "initiatives": _page(
                [
                    {"id": f"initiative-{wave}", "name": wave.upper(), "description": ""}
                    for wave in ("a", "b")
                ]
            )
        }
    if operation == "ListInitiativeProjects":
        return {
            "initiative": {
                "projects": _page(
                    [
                        project
                        for project in projects.values()
                        if {"id": variables["initiativeId"]} in project["initiatives"]["nodes"]
                    ]
                )
            }
        }
    if operation == "ListProjectIssues":
        return {
            "project": {
                "issues": _page([issue] if issue["project"]["id"] == variables["projectId"] else [])
            }
        }
    if operation == "FindProject":
        return {"projects": _page([projects[identity]] if identity in projects else [])}
    if operation == "ProjectOwnership":
        return {"project": projects.get(identity)}
    if operation in ("IssueOwnership", "IssueObservation", "IssueComments", "IssueTeam"):
        assert identity in ("issue-1", "FIX-1"), variables
        return {
            "issue": {
                **issue,
                "project": projects[issue["project"]["id"]],
                "updatedAt": "2026-09-28T00:00:00Z",
                "comments": _page([]),
            }
        }
    if operation == "ProjectStatuses":
        return {
            "projectStatuses": _page(
                [
                    {"id": kind, "type": kind, "teamId": None, "position": 0.0}
                    for kind in ("planned", "started", "completed")
                ]
            )
        }
    if operation == "SetProjectStatus":
        projects[identity]["status"]["type"] = variables["statusId"]
        return {"projectUpdate": {"success": True}}
    if operation == "MoveIssueToProject":
        assert identity == issue["id"]
        target = projects[variables["projectId"]]
        issue["project"] = {"id": target["id"], "name": target["name"]}
        return {"issueUpdate": {"issue": {"id": identity}}}
    # No catch-all mutation success: an unexpected provider call fails the proof.
    raise AssertionError(f"Unsupported synthetic Linear operation: {operation}")


def _provider() -> None:
    """Finite scripted Codex; the real lf worker owns admission and completion."""
    if "--version" in sys.argv:
        print("codex-cli chapter-fixture")
        return
    if "--listen" in sys.argv and not os.environ.get("LF_TEST_CODEX_STDIO"):
        pid = os.getpid()
        stamp = subprocess.check_output(["ps", "-p", str(pid), "-o", "lstart="], text=True).strip()
        Path(os.environ["CHAPTER_ENGINE_RECEIPT"]).write_text(json.dumps([pid, stamp]))
        bridge = Path(__file__).resolve().parents[2] / "rust/loopflow/tests/support/codex_socket.py"
        os.execv(sys.executable, [sys.executable, str(bridge), sys.argv[0], *sys.argv[1:]])
    thread_id = "chapter-thread"
    for line in sys.stdin:
        request = json.loads(line)
        method = request["method"]
        if "id" not in request:
            assert method == "initialized", method
            continue
        result = {}
        if method == "account/read":
            result = {"account": {"type": "chatgpt", "email": "fixture@example.test"}}
        elif method in ("thread/start", "thread/resume"):
            result = {"thread": {"id": thread_id}}
        elif method == "turn/start":
            prompt = json.dumps(request["params"])
            assert MARKER in prompt, "Project-authored skill was not sent to the provider"
            receipt = Path(os.environ["CHAPTER_PROVIDER_RECEIPT"])
            assert not receipt.exists(), "unexpected second provider turn"
            receipt.write_text(
                json.dumps(
                    {
                        "pid": os.getpid(),
                        "cwd": str(Path.cwd()),
                        "request": request,
                        "database": os.environ["LF_DB_PATH"],
                    }
                )
            )
            result = {"turn": {"id": "chapter-turn"}}
        else:
            assert method == "initialize", f"Unsupported Codex method: {method}"
        print(json.dumps({"id": request["id"], "result": result}), flush=True)
        if method == "turn/start":
            print(
                json.dumps(
                    {
                        "method": "turn/started",
                        "params": {
                            "threadId": thread_id,
                            "turn": {"id": "chapter-turn", "status": "inProgress"},
                        },
                    }
                ),
                flush=True,
            )
            print(
                json.dumps(
                    {
                        "method": "item/agentMessage/delta",
                        "params": {
                            "threadId": thread_id,
                            "turnId": "chapter-turn",
                            "itemId": "chapter-message",
                            "delta": "Chapter fixture complete.",
                        },
                    }
                ),
                flush=True,
            )
            print(
                json.dumps(
                    {
                        "method": "turn/completed",
                        "params": {
                            "threadId": thread_id,
                            "turn": {"id": "chapter-turn", "status": "completed"},
                        },
                    }
                ),
                flush=True,
            )
            return


def _stop_provider(root: Path) -> None:
    receipt = root / "engine.json"
    if not receipt.exists():
        return
    pid, stamp = json.loads(receipt.read_text())
    observed = subprocess.run(
        ["ps", "-p", str(pid), "-o", "lstart="], capture_output=True, text=True, timeout=5
    )
    if observed.returncode != 0 or observed.stdout.strip() != stamp:
        return
    try:
        assert os.getpgid(pid) == pid, "fixture provider must own its process group"
        os.killpg(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        observed = subprocess.run(
            ["ps", "-p", str(pid), "-o", "lstart=", "-o", "stat="],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if observed.returncode != 0 or not observed.stdout.strip().startswith(stamp):
            return
        if "Z" in observed.stdout.split()[-1]:
            return
        time.sleep(0.05)
    raise AssertionError(f"Fixture provider {pid} did not stop; discard the proof container")


def _run(
    argv: list[str], cwd: Path, env: dict[str, str], logs: Path
) -> subprocess.CompletedProcess:
    result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=90)
    with logs.open("a") as output:
        output.write(
            json.dumps(
                {
                    "argv": argv,
                    "code": result.returncode,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                }
            )
            + "\n"
        )
    assert result.returncode == 0, f"{shlex.join(argv)}\n{result.stdout}\n{result.stderr}"
    return result


def _snapshot(database: Path) -> dict:
    with sqlite3.connect(f"file:{database}?mode=ro", uri=True) as db:
        db.row_factory = sqlite3.Row
        task = dict(db.execute("SELECT * FROM tasks WHERE external_issue_id='issue-1'").fetchone())
        flow = dict(
            db.execute(
                "SELECT * FROM flow_sessions WHERE id=?", (task["current_invocation_id"],)
            ).fetchone()
        )
        prs = [
            dict(row)
            for row in db.execute(
                "SELECT * FROM task_prs WHERE task_id=? ORDER BY sequence", (task["id"],)
            )
        ]
        assert db.execute("PRAGMA foreign_key_check").fetchall() == []
        return {"task": task, "flow": flow, "prs": prs}


def _preserved(before: dict, after: dict) -> None:
    # Only planning freshness and Project membership may change on sync.
    ignored = {"project_id", "updated_at", "pm_snapshot_synced_at"}
    assert {k: v for k, v in after["task"].items() if k not in ignored} == {
        k: v for k, v in before["task"].items() if k not in ignored
    }
    assert after["task"]["started_at"] == before["task"]["started_at"] is not None
    assert after["flow"] == before["flow"], "sync changed captured execution/review"
    assert after["prs"] == before["prs"], "sync changed PR identity/publication/placement"


def _planning(database: Path, task_project: str) -> None:
    with sqlite3.connect(f"file:{database}?mode=ro", uri=True) as db:
        assert db.execute(
            "SELECT external_project_id,status,flow FROM projects ORDER BY external_project_id"
        ).fetchall() == [
            (f"{wave}-{suffix}", status, flow)
            for wave in ("a", "b")
            for suffix, status, flow in (
                ("next", "started", "successor-proof"),
                ("old", "completed", FLOW),
            )
        ]
        assert db.execute(
            "SELECT external_project_id FROM projects WHERE id=?", (task_project,)
        ).fetchone() == ("a-next",)
        snapshots = [json.loads(row[0]) for row in db.execute("SELECT payload FROM pm_snapshots")]
        assert len(snapshots) == 2
        for snapshot in snapshots:
            current = [p for p in snapshot["projects"] if p["status"] == "started"]
            assert len(current) == 1 and current[0]["name"].endswith("next")
        assert any(
            item["id"] == "issue-1" and item["project_id"] == "a-next"
            for snapshot in snapshots
            for item in snapshot["items"]
        )


def _exercise(lf: Path, root: Path, env: dict[str, str], server: ThreadingHTTPServer) -> dict:
    repo, first, second = root / "repo", root / "home-a", root / "home-b"
    repo.mkdir()
    first.mkdir()
    second.mkdir()
    logs = root / "commands.jsonl"
    env.update(
        LF_HOME=str(first),
        LF_DB_PATH=str(first / "loopflow.db"),
        LF_CONTROL_HOME=str(first),
        LF_CONTROL_DB_PATH=str(first / "loopflow.db"),
    )

    def _git(*args: str) -> subprocess.CompletedProcess:
        return _run(["git", *args], repo, env, logs)

    _git("init", "-q", "-b", "main")
    _git("config", "user.name", "Fixture")
    _git("config", "user.email", "fixture@example.test")
    (repo / ".lf/flows").mkdir(parents=True)
    (repo / ".lf/skills").mkdir()
    (repo / ".lf/config.yaml").write_text(
        "agent: codex\npm:\n  provider: linear\n  linear_team: team-1\n"
    )
    (repo / f".lf/flows/{FLOW}.yaml").write_text(
        "- chapter-marker\n- step:\n    id: review\n    name: chapter-review\n    human: true\n"
    )
    (repo / ".lf/flows/successor-proof.yaml").write_text("- chapter-marker\n")
    (repo / ".lf/skills/chapter-marker.md").write_text(f"Record {MARKER}.\n")
    (repo / ".lf/skills/chapter-review.md").write_text("Review the retained Chapter proof.\n")
    for wave in ("a", "b"):
        path = repo / f"wave/{wave}"
        path.mkdir(parents=True)
        (path / "GOAL.md").write_text(
            f"---\npm:\n  linear_initiative: initiative-{wave}\n---\nPreserve work.\n"
        )
    _git("add", ".")
    _git("commit", "-qm", "Chapter fixture")
    remote = root / "remotes/loopflowstudio/fixture.git"
    remote.parent.mkdir(parents=True)
    _git("clone", "--bare", str(repo), str(remote))
    _git("remote", "add", "origin", str(remote))
    _git("fetch", "origin")
    _git("remote", "set-head", "origin", "main")

    def _cli(*args: str) -> subprocess.CompletedProcess:
        return _run([str(lf), *args], repo, env, logs)

    _cli("session", "list", "--json")  # Initialize the candidate's own schema.
    key = AESGCM.generate_key(bit_length=256)
    key_path = root / "synthetic-provider.key"
    key_path.write_bytes(base64.b64encode(key).rstrip(b"="))
    key_path.chmod(0o600)
    env["LF_PROVIDER_TOKEN_KEY_PATH"] = str(key_path)
    provider_home = root / "codex"
    provider_home.mkdir()
    (provider_home / "auth.json").write_text(
        json.dumps(
            {
                "access_token": "synthetic-chapter-token",
                "expires_at": int(time.time()) + 86400,
            }
        )
    )
    (provider_home / "auth.json").chmod(0o600)
    env["CODEX_HOME"] = str(provider_home)
    with sqlite3.connect(env["LF_DB_PATH"]) as db:
        db.execute("PRAGMA foreign_keys=ON")
        for wave in ("a", "b"):
            db.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?,?,?,?)",
                (str(uuid.uuid4()), wave, str(repo), int(time.time())),
            )
        db.execute(
            "INSERT INTO provider_tokens"
            "(provider,access_token,updated_at,credential_type,encrypted) "
            "VALUES('linear',?,?,'apikey',1)",
            (_encrypt(key, "synthetic-linear-token"), int(time.time())),
        )
        # Synthetic account setup is distinct from execution evidence: no Exec,
        # Task, Session, Flow, Started, or completion row is seeded here.
        db.execute(
            "INSERT INTO provider_accounts(provider,account_id,home,credential_state,"
            "routing_state,created_at,updated_at) VALUES('codex','fixture',?,'connected',"
            "'automatic',?,?)",
            (str(provider_home), int(time.time()), int(time.time())),
        )
    _cli("wave", "sync", "--all")
    _cli("task", "checkout", "FIX-1", "--name", "chapter-task", "--json")
    with sqlite3.connect(env["LF_DB_PATH"]) as db:
        assert db.execute("SELECT started_at FROM tasks").fetchall() == [(None,)]
    _cli("task", "run", "FIX-1", "--json")  # Deliberately no --flow, even on checkout.
    deadline = time.monotonic() + 45
    while True:
        with sqlite3.connect(env["LF_DB_PATH"]) as db:
            flow = db.execute(
                "SELECT f.pending_session_id,f.claim_json,f.failure_json "
                "FROM flow_sessions f JOIN tasks t ON t.current_invocation_id=f.id"
            ).fetchone()
            workers = db.execute(
                "SELECT outcome FROM execs WHERE command LIKE '%__worker%'"
            ).fetchall()
        assert not (flow and flow[2]), f"Task worker failed: {flow[2]}"
        if flow and flow[0] and flow[1] is None and workers and all(row[0] for row in workers):
            assert all(row[0] == "succeeded" for row in workers), workers
            break
        assert time.monotonic() < deadline, f"Task did not park at review: {flow}; Execs={workers}"
        time.sleep(0.1)
    receipt = json.loads((root / "provider.json").read_text())
    before = _snapshot(first / "loopflow.db")
    assert receipt["cwd"] == before["task"]["worktree"]
    assert receipt["database"] == str(first / "loopflow.db")
    assert MARKER in json.dumps(receipt["request"])
    assert json.loads(before["flow"]["invocation_json"])["flow"] == FLOW
    assert before["task"]["started_at"] is not None
    with sqlite3.connect(first / "loopflow.db") as db:
        consumed = db.execute(
            "SELECT s.payload,e.command,e.outcome FROM flow_events f "
            "JOIN session_events s ON s.seq=f.session_event "
            "JOIN session_events origin ON origin.session_id=s.session_id "
            "AND origin.provider_thread=s.provider_thread AND origin.provider_turn=s.provider_turn "
            "AND origin.kind='started' "
            "JOIN execs e ON e.id=origin.exec_id "
            "WHERE f.flow_id=? AND f.kind='consumed'",
            (before["flow"]["id"],),
        ).fetchall()
        assert len(consumed) == 1, consumed
        assert json.loads(consumed[0][0])["status"] == "completed", consumed
        assert "__worker" in consumed[0][1] and consumed[0][2] == "succeeded", consumed
        (root / "default-launch.json").write_text(
            json.dumps(
                {
                    "flow": FLOW,
                    "provider": receipt,
                    "consumed": consumed,
                    "started_at": before["task"]["started_at"],
                },
                indent=2,
            )
            + "\n"
        )
        # Give preservation a nonempty published PR; this is fixture state,
        # never evidence of a real GitHub publication.
        db.execute(
            "UPDATE task_prs SET publication_requested_at=123,github_number=17,"
            "github_url='https://github.com/loopflowstudio/fixture/pull/17'"
        )
        db.commit()
        with sqlite3.connect(second / "loopflow.db") as destination:
            db.backup(destination)
    before = _snapshot(first / "loopflow.db")
    assert before["prs"] and all(pr["github_number"] == 17 for pr in before["prs"]), before["prs"]
    assert _snapshot(second / "loopflow.db") == before
    checkout = Path(before["task"]["worktree"])
    authored = checkout / "retained.txt"
    authored.write_text("Preserve this unfinished Task across the chapter.\n")
    provider_before = copy.deepcopy(server.state["projects"])
    _cli("repo", "new-chapter", "next", "--json")
    first_after = _snapshot(first / "loopflow.db")
    _preserved(before, first_after)
    _planning(first / "loopflow.db", first_after["task"]["project_id"])
    assert _snapshot(second / "loopflow.db") == before, "Home B changed before its sync"
    assert server.state["issue"]["project"]["id"] == "a-next"
    for identity, project in provider_before.items():
        assert server.state["projects"][identity]["content"] == project["content"]
    after_rotation = copy.deepcopy(server.state)
    env.update(
        LF_HOME=str(second),
        LF_DB_PATH=str(second / "loopflow.db"),
        LF_CONTROL_HOME=str(second),
        LF_CONTROL_DB_PATH=str(second / "loopflow.db"),
    )
    _cli("wave", "sync", "--all")  # The sole adoption command in Home B.
    after = _snapshot(second / "loopflow.db")
    _preserved(before, after)
    assert after["task"]["project_id"] != before["task"]["project_id"]
    _planning(second / "loopflow.db", after["task"]["project_id"])
    assert server.state["projects"] == after_rotation["projects"]
    assert server.state["issue"] == after_rotation["issue"]
    sync_operations = server.state["operations"][len(after_rotation["operations"]) :]
    assert sync_operations and all(
        operation.startswith(("List", "Find", "Issue", "Project")) for operation in sync_operations
    ), sync_operations
    assert authored.read_text() == "Preserve this unfinished Task across the chapter.\n"
    assert not server.errors, server.errors
    return {
        "default_launch": "passed",
        "sync_only_adoption": "passed",
        "before": before,
        "after": after,
        "consumed": consumed,
        "sync_operations": sync_operations,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert sys.platform == "linux", "SSL_CERT_FILE proof requires Linux"
    # Task destination ignores HOME. Never execute this against a host installation.
    account_home = Path(pwd.getpwuid(os.getuid()).pw_dir)
    assert not (account_home / ".lf").exists(), (
        "Use a fresh disposable OS account without an installation"
    )
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / "bin").mkdir()
    binary = root / "bin/lf"
    shutil.copy2(args.lf.resolve(), binary)
    script = Path(__file__).resolve()
    codex = root / "bin/codex"
    codex.write_text(
        f"#!{sys.executable}\nimport runpy, sys\nsys.path.insert(0, {str(script.parent)!r})\n"
        f"runpy.run_path({str(script)!r}, run_name='fixture')['_provider']()\n"
    )
    codex.chmod(0o700)
    tmux = shutil.which("tmux")
    assert tmux, "Install tmux in the disposable container"
    socket_name = "chapter-" + uuid.uuid4().hex
    # Keep the fixture provider on PATH after tmux's /bin/sh -lc reads /etc/profile.
    # All worker arguments and the real Task driver remain unchanged.
    fixture_path = f"{root / 'bin'}:{os.environ['PATH']}"
    (root / "bin/tmux").write_text(
        f"#!{sys.executable}\nimport os, sys\nargs = sys.argv[1:]\n"
        f"if args and args[0] == 'new-session':\n"
        f"    args[-1] = {('export PATH=' + shlex.quote(fixture_path) + '; ')!r} + args[-1]\n"
        f"os.execv({tmux!r}, [{tmux!r}, '-L', {socket_name!r}, *args])\n"
    )
    (root / "bin/tmux").chmod(0o700)
    (root / "empty-certs").mkdir()
    (root / "user").mkdir()
    # Positive allowlist excludes every inherited LF_*, LOOPFLOW_*, credential,
    # proxy, Git configuration override, and provider account Home.
    env = {
        "PATH": f"{root / 'bin'}:{os.environ['PATH']}",
        "HOME": str(root / "user"),
        "LANG": "C.UTF-8",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_TERMINAL_PROMPT": "0",
        "LF_BIN": str(binary),
        "LF_CONTROL_BIN": str(binary),
        "SSL_CERT_FILE": str(root / "certificate.pem"),
        "SSL_CERT_DIR": str(root / "empty-certs"),
        "CHAPTER_PROVIDER_RECEIPT": str(root / "provider.json"),
        "CHAPTER_ENGINE_RECEIPT": str(root / "engine.json"),
    }
    result = {
        "status": "failed",
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "scope": "public CLI, synthetic Linear/Codex, disposable Linux account",
    }
    with ThreadingHTTPServer(("127.0.0.1", 0), ChapterHandler) as server:
        server.tls = _tls_context(root)
        server.state_lock = threading.Lock()
        server.errors = []
        projects = {}
        for wave in ("a", "b"):
            for suffix, status, flow in (
                ("old", "started", FLOW),
                ("next", "planned", "successor-proof"),
            ):
                identity = f"{wave}-{suffix}"
                projects[identity] = {
                    "id": identity,
                    "name": f"{wave.upper()} — {'previous' if suffix == 'old' else 'next'}",
                    "description": "Preserve the authored plan",
                    "content": f"flow: {flow}\n\n## KRs\n- [ ] Retain proof",
                    "archivedAt": None,
                    "status": {"type": status},
                    "teams": {"nodes": [{"id": "team-1"}]},
                    "initiatives": {"nodes": [{"id": f"initiative-{wave}"}]},
                }
        server.state = {
            "projects": projects,
            "operations": [],
            "issue": {
                "id": "issue-1",
                "identifier": "FIX-1",
                "title": "Chapter task",
                "description": "Retain execution.",
                "url": None,
                "sortOrder": 1.0,
                "prioritySortOrder": 1.0,
                "assignee": None,
                "state": {"type": "started"},
                "team": {"id": "team-1"},
                "trashed": False,
                "project": {"id": "a-old", "name": "A — previous"},
            },
        }
        env["HTTPS_PROXY"] = f"http://127.0.0.1:{server.server_port}"
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            result.update(_exercise(binary, root, env, server), status="passed")
        except BaseException as error:
            result["error"] = str(error)
            raise
        finally:
            try:
                subprocess.run(
                    [tmux, "-L", socket_name, "kill-server"],
                    env=env,
                    capture_output=True,
                    timeout=10,
                )
                _stop_provider(root)
            except BaseException as error:
                result.update(status="failed", cleanup_error=str(error))
                raise
            finally:
                server.shutdown()
                thread.join(timeout=5)
                result["provider_operations"] = server.state["operations"]
                result["provider_errors"] = server.errors
                (root / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "receipt": str(root / "results.json")}))


if __name__ == "__main__":
    main()
