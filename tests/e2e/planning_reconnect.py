"""Planning refresh and Graph-boundary recovery against a disposable Linear HTTPS peer."""

import copy
import hashlib
import json
import os
import queue
import sqlite3
import subprocess
import sys
import threading
import time
import uuid
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from linear_fixture import tls_context


def _page(nodes: list) -> dict:
    return {"nodes": nodes, "pageInfo": {"hasNextPage": False, "endCursor": None}}


def _revision() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _comment(body: str, issue: str, identity: str | None = None) -> dict:
    return {
        "id": identity or str(uuid.uuid4()),
        "body": body,
        "issue": {"id": issue},
        "createdAt": _revision(),
        "updatedAt": _revision(),
        "user": {"id": "fixture-person", "name": "Maya", "displayName": "Maya"},
    }


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass

    def do_CONNECT(self) -> None:
        assert self.path == "api.linear.app:443"
        self.send_response(200)
        self.end_headers()
        self.connection = self.server.tls.wrap_socket(self.connection, server_side=True)
        self.rfile = self.connection.makefile("rb")
        self.wfile = self.connection.makefile("wb")
        self.close_connection = False

    def do_POST(self) -> None:
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with self.server.lock:
            response = self._respond(request["query"], request.get("variables", {}))
        body = json.dumps(response).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _respond(self, query: str, variables: dict) -> dict:
        state = self.server.state
        state["requests"] += 1
        if state["offline"]:
            return {"errors": [{"message": "fixture offline"}]}
        issue = next((i for i in state["issues"] if i["id"] == variables.get("id")), None)
        project = state["project"]
        if "query TeamChanges" in query:

            def eligible(item: dict) -> bool:
                selected = variables["issues"]
                return (
                    item["team"]["id"] == selected["team"]["id"]["eq"]
                    and item["id"] not in selected["id"]["nin"]
                    and (
                        item["id"] in selected["or"][0]["id"]["in"]
                        or item["state"]["type"] not in ("completed", "canceled")
                    )
                )

            def recent(item: dict, selected: dict) -> bool:
                return selected.get("gte", "") <= item["updatedAt"] <= selected["lte"]

            issues = {item["id"]: item for item in state["issues"] if eligible(item)}
            data = {
                "issues": _page(
                    [
                        item
                        for item in issues.values()
                        if recent(item, variables["issues"]["updatedAt"])
                    ]
                ),
                "comments": _page(
                    [
                        {**comment, "issue": issues[comment["issue"]["id"]]}
                        for comment in state["comments"]
                        if comment["issue"]["id"] in issues
                        and recent(comment, variables["comments"]["updatedAt"])
                    ]
                ),
            }
        elif "query ListTeams" in query:
            data = {
                "teams": _page(
                    [
                        {
                            "id": "team-task-pr-tests",
                            "name": "Fixture",
                            "key": "INF",
                            "description": "<!-- loopflow-repository: loopflowstudio/fixture -->",
                        }
                    ]
                )
            }
        elif "query ListInitiatives" in query:
            data = {
                "initiatives": _page(
                    [
                        {
                            "id": "initiative-task-pr-tests",
                            "name": state["initiative_name"],
                            "description": state.get("initiative_description", ""),
                        }
                    ]
                )
            }
        elif "mutation UpdateInitiative" in query:
            update = variables["input"]
            state["initiative_name"] = update.get("name", state["initiative_name"])
            state["initiative_description"] = update.get(
                "description", state.get("initiative_description", "")
            )
            data = {
                "initiativeUpdate": {
                    "success": True,
                    "initiative": {"id": variables["id"]},
                }
            }
        elif "query ListInitiativeProjects" in query:
            data = {"initiative": {"projects": _page([project])}}
        elif "query ListProjectIssues" in query:
            data = {"project": {"issues": _page(state["issues"])}}
        elif "query ProjectOwnership" in query:
            data = {"project": project}
        elif "query FindProject" in query:
            data = {"projects": _page([project])}
        elif "query IssueOwnership" in query:
            data = {"issue": issue}
        elif "query IssueObservation" in query or "query IssueComments" in query:
            data = {
                "issue": {
                    **issue,
                    "comments": _page(
                        [c for c in state["comments"] if c["issue"]["id"] == issue["id"]]
                    ),
                }
            }
        elif "query IssueTeam" in query:
            data = {"issue": {"team": issue["team"]}}
        elif "query WorkflowStates" in query:
            data = {"workflowStates": _page([{"id": variables["type"], "position": 0}])}
        elif "mutation SetIssueState" in query:
            issue["state"] = {"type": variables["stateId"]}
            issue["updatedAt"] = _revision()
            issue["completedAt"] = (
                issue["updatedAt"] if variables["stateId"] == "completed" else None
            )
            state["state_writes"] += 1
            # Commit, then lose the response. Never send this mutation twice.
            return {"errors": [{"message": "lost state response"}]}
        elif "mutation DeliverTaskField" in query:
            return {"errors": [{"message": "unrelated title write rejected"}]}
        elif "mutation SyncComment" in query:
            if not any(c["id"] == variables["id"] for c in state["comments"]):
                state["comments"].append(
                    _comment(variables["body"], variables["issueId"], variables["id"])
                )
            return {"errors": [{"message": "lost comment response"}]}
        elif "query CommentDelivery" in query:
            data = {
                "comment": next((c for c in state["comments"] if c["id"] == variables["id"]), None)
            }
        else:
            state["unexpected"].append(query)
            return {"errors": [{"message": f"unexpected fixture operation: {query}"}]}
        return {"data": data}


def _await(check, message: str, seconds: int = 45) -> None:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.05)
    raise AssertionError(message)


class Watch:
    """One `lf <args> --json --watch`, a reader as an open Desktop window is.
    Readers show stored planning; they exchange nothing."""

    def __init__(self, fixture: dict, env: dict, *args: str):
        self.process = subprocess.Popen(
            [fixture["lf"], *args, "--json", "--watch"],
            cwd=fixture["repo"],
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        self.frames = queue.Queue()
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()

    def _read(self) -> None:
        for line in self.process.stdout:
            self.frames.put(json.loads(line))

    def frame(self, matches) -> dict:
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            frame = self.frames.get(timeout=max(0.01, deadline - time.monotonic()))
            if matches(frame):
                return frame
        raise AssertionError("Desktop did not receive the expected frame")

    def close(self) -> None:
        self.process.stdin.close()
        try:
            assert self.process.wait(timeout=10) == 0
        finally:
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait()
            self.reader.join(timeout=5)


def _exercise(fixture: dict, env: dict, server: ThreadingHTTPServer, mode: str) -> None:
    root, repo = Path(fixture["home"]), Path(fixture["repo"])
    db = sqlite3.connect(root / "loopflow.db", timeout=5)

    def run(*args: str) -> str:
        result = subprocess.run(
            [fixture["lf"], *args], cwd=repo, env=env, capture_output=True, text=True, timeout=30
        )
        assert result.returncode == 0, result.stderr
        return result.stdout

    def refresh_until(check, message: str) -> None:
        """Desktop's planning clock: `lf wave status --sync` until `check` holds."""
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            run("wave", "status", fixture["wave"], "--sync")
            if check():
                return
            time.sleep(1)
        raise AssertionError(message)

    # Acquire a real baseline before the outage, not an invented provider revision.
    run("wave", "status", fixture["wave"], "--sync")
    remote_id = db.execute(
        "SELECT id FROM tasks WHERE external_issue_id=?", (server.state["issues"][2]["id"],)
    ).fetchone()[0]
    local_id = db.execute("SELECT id FROM tasks WHERE issue_identifier='INF-124'").fetchone()[0]
    prs = db.execute("SELECT id,task_id,branch,base_commit FROM task_prs").fetchall()
    watch = None
    graph = None
    graph_log = None
    try:
        with server.lock:
            server.state["offline"] = True
        if mode == "graph":
            # Graph admission requires a managed login, even with a contained provider.
            profile = root / "accounts/claude/reconnect"
            profile.mkdir(parents=True)
            credential = json.dumps(
                {"claudeAiOauth": {"accessToken": "fixture-reconnect", "expiresAt": 4102444800000}}
            )
            (profile / ".credentials.json").write_text(credential)
            now = int(time.time())
            db.execute(
                "INSERT INTO provider_accounts(provider,account_id,home,login_email,"
                "credential_state,routing_state,created_at,updated_at,observed_email,"
                "observed_subject,observed_credential_digest) "
                "VALUES('claude',?,?,?,'connected','automatic',?,?,?,?,?)",
                (
                    "reconnect",
                    str(profile),
                    "reconnect@example.com",
                    now,
                    now,
                    "reconnect@example.com",
                    "reconnect",
                    hashlib.sha256(credential.encode()).hexdigest(),
                ),
            )
            db.commit()
            (repo / ".lf/graphs").mkdir(exist_ok=True)
            (repo / ".lf/skills").mkdir(exist_ok=True)
            (repo / ".lf/graphs/reconnect.yaml").write_text("- reconnect-proof\n- cmd: context\n")
            (repo / ".lf/skills/reconnect-proof.md").write_text(
                "---\nagent: claude\n---\nWait for the fixture.\n"
            )
            graph_log = (root / "graph.log").open("w+")
            graph = subprocess.Popen(
                [fixture["lf"], "-b", "--task", "INF-123", "graph", "reconnect"],
                cwd=repo,
                env=env,
                stdout=graph_log,
                stderr=graph_log,
            )
            _await(
                lambda: (root / "provider-started").exists() or graph.poll() is not None,
                "Graph never launched its provider",
            )
            assert graph.poll() is None, (root / "graph.log").read_text()
        run("task", "edit", "INF-123", "--title", "Pending local title")
        run("task", "complete", "INF-124", "--reason", "Saved offline")
        saved = json.loads(run("task", "comment", "INF-124", "Saved offline", "--json"))
        comment_id = saved["pending_sync"][0]
        receipt = db.execute(
            "SELECT id FROM task_state_deliveries WHERE task_id=? ORDER BY seq DESC LIMIT 1",
            (local_id,),
        ).fetchone()[0]
        task_graphs = db.execute("SELECT * FROM task_graphs ORDER BY task_id").fetchall()
        _await(
            lambda: db.execute(
                "SELECT error FROM task_state_deliveries WHERE id=?", (receipt,)
            ).fetchone()[0],
            "active connection did not observe outage",
        )
        with server.lock:
            state = server.state
            state["offline"] = False
            state["issues"][2]["state"] = {"type": "completed"}
            state["issues"][2]["completedAt"] = _revision()
            state["issues"][2]["updatedAt"] = _revision()
            added = copy.deepcopy(state["issues"][0])
            added.update(id=str(uuid.uuid4()), identifier="INF-126", title="Added while offline")
            added["updatedAt"] = _revision()
            state["issues"].append(added)
            incoming = _comment("Incoming without Task selection", state["issues"][2]["id"])
            state["comments"].append(incoming)

        def caught_up() -> bool:
            return (
                db.execute(
                    "SELECT settled FROM task_state_deliveries WHERE id=?", (receipt,)
                ).fetchone()
                == (1,)
                and db.execute(
                    "SELECT acknowledged FROM task_comment_deliveries WHERE comment_id=?",
                    (comment_id,),
                ).fetchone()
                == (1,)
                and db.execute(
                    "SELECT count(*) FROM task_comments WHERE id=?", (incoming["id"],)
                ).fetchone()
                == (1,)
                and db.execute(
                    "SELECT count(*) FROM tasks WHERE issue_identifier='INF-126'"
                ).fetchone()
                == (1,)
                and db.execute(
                    "SELECT planning_completed FROM tasks WHERE id=?",
                    (remote_id,),
                ).fetchone()
                == (1,)
            )

        if graph:
            # Agents have no Linear timer. End this turn, then let the next Graph
            # step resolve the lost write replies using their retained identities.
            with server.lock:
                requests = server.state["requests"]
            attempted = db.execute("SELECT max(attempted_at) FROM linear_team_checks").fetchone()[0]
            _await(lambda: time.time() >= attempted + 11, "team freshness never expired")
            with server.lock:
                assert server.state["requests"] == requests, "idle agent polled Linear"
            assert not caught_up()
            (root / "provider-stop").touch()
            assert graph.wait(timeout=30) == 0, (root / "graph.log").read_text()
            assert caught_up(), "Graph boundaries did not acquire and settle pending planning"
        else:
            # Acquisition and uncertain-write settlement may need separate refreshes.
            refresh_until(caught_up, "planning refresh did not catch up after reconnect")
        assert db.execute("SELECT * FROM task_graphs ORDER BY task_id").fetchall() == task_graphs
        assert db.execute("SELECT id,task_id,branch,base_commit FROM task_prs").fetchall() == prs
        assert db.execute("SELECT worktree FROM tasks WHERE id=?", (remote_id,)).fetchone() == (
            None,
        )
        assert db.execute(
            "SELECT count(*) FROM task_changes WHERE task_id=? AND field='task_title' "
            "AND conflict_json IS NULL AND acknowledged=0",
            (fixture["task"],),
        ).fetchone() == (1,)
        with server.lock:
            assert server.state["state_writes"] == 1
            assert len([c for c in server.state["comments"] if c["id"] == comment_id]) == 1
            assert not server.state["unexpected"], server.state["unexpected"]
        if not graph:
            planning = Watch(fixture, env, "wave", "show", "--all")
            planning.frame(lambda f: "INF-126" in json.dumps(f["body"]))
            planning.close()
            task = Watch(fixture, env, "task", "status", "INF-125")
            task.frame(lambda f: incoming["id"] in json.dumps(f["body"]))
            task.close()
            # A short command attempts delivery itself. An outage leaves the save
            # pending; an open reader never delivers it, and the next refresh does.
            with server.lock:
                state["offline"] = True
            gap = json.loads(run("task", "comment", "INF-124", "Saved during an outage", "--json"))[
                "pending_sync"
            ][0]
            with server.lock:
                assert not any(c["id"] == gap for c in state["comments"])
                state["offline"] = False

            def delivered() -> bool:
                return db.execute(
                    "SELECT acknowledged FROM task_comment_deliveries WHERE comment_id=?", (gap,)
                ).fetchone() == (1,)

            watch = Watch(fixture, env, "planning", "status")
            watch.frame(lambda f: True)
            time.sleep(2)
            assert not delivered(), "a planning reader delivered a pending save"
            refresh_until(delivered, "planning refresh did not deliver a pending save")
        else:
            assert db.execute("SELECT count(*) FROM graph_process_steps").fetchone()[0] == 2
    finally:
        (root / "provider-stop").touch()
        if watch:
            watch.close()
        if graph and graph.poll() is None:
            try:
                graph.wait(timeout=20)
            except subprocess.TimeoutExpired:
                graph.kill()
                graph.wait()
        if graph_log:
            graph_log.close()
        db.close()


def main() -> None:
    fixture = json.loads(Path(sys.argv[1]).read_text())
    root, repo = Path(fixture["home"]), Path(fixture["repo"])
    tls, cert = tls_context(root)
    subprocess.run(
        ["git", "remote", "set-url", "origin", "https://github.com/loopflowstudio/fixture.git"],
        cwd=repo,
        check=True,
    )
    project = {
        "id": fixture["project"],
        "name": "Task PR tests",
        "description": "",
        "content": "",
        "updatedAt": "2026-10-08T12:00:00Z",
        "archivedAt": None,
        "status": {"type": "started"},
        "initiatives": _page([{"id": "initiative-task-pr-tests"}]),
        "teams": _page([{"id": "team-task-pr-tests"}]),
    }
    issue = {
        "id": fixture["issue"],
        "identifier": "INF-123",
        "title": "Prove Task PR transitions",
        "description": "Exercise the persisted lifecycle.",
        "url": None,
        "updatedAt": "2026-10-08T12:00:00Z",
        "sortOrder": 1,
        "prioritySortOrder": 1,
        "assignee": None,
        "completedAt": None,
        "dueDate": None,
        "state": {"type": "unstarted"},
        "team": {"id": "team-task-pr-tests"},
        "project": project,
    }
    other = copy.deepcopy(issue)
    other.update(id=str(uuid.uuid4()), identifier="INF-124", title="Outgoing unplaced work")
    remote = copy.deepcopy(issue)
    remote.update(id=str(uuid.uuid4()), identifier="INF-125", title="Incoming unplaced work")
    bin_dir = root / "bin"
    bin_dir.mkdir()
    provider = bin_dir / "claude"
    provider.write_text("""#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

if "--version" in sys.argv:
    print("claude test")
    sys.exit(0)
root = Path(os.environ["HOME"])
for line in sys.stdin:
    message = json.loads(line)
    if message.get("type") != "user":
        continue
    message["session_id"] = "reconnect-thread"
    print(json.dumps(message), flush=True)
    (root / "provider-started").touch()
    for _ in range(900):
        if (root / "provider-stop").exists():
            break
        time.sleep(0.1)
    print(json.dumps({"type": "result", "uuid": message["uuid"],
        "session_id": "reconnect-thread", "subtype": "success",
        "result": "Done"}), flush=True)
""")
    provider.chmod(0o755)
    for name in ["codex", "gh", "opencode"]:
        stub = bin_dir / name
        stub.write_text("#!/bin/sh\nexit 1\n")
        stub.chmod(0o755)
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith(
            ("LF_", "LOOPFLOW_", "LINEAR_", "CLAUDE_", "CODEX_", "ANTHROPIC_", "OPENAI_")
        )
        and k.lower() not in ("http_proxy", "https_proxy", "all_proxy", "no_proxy")
    }
    env.update(
        HOME=str(root),
        LF_HOME=str(root),
        LF_BIN=fixture["lf"],
        LF_USER_NAME="Fixture Person",
        PATH=f"{bin_dir}:/usr/bin:/bin",
        LF_PROVIDER_TOKEN_KEY_PATH=str(root / "provider.key"),
        SSL_CERT_FILE=str(cert),
        SSL_CERT_DIR=str(root / "empty-certs"),
        CLAUDE_CONFIG_DIR=str(root / "claude"),
        CODEX_HOME=str(root / "codex"),
    )
    (root / "empty-certs").mkdir()
    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        server.tls = tls
        server.lock = threading.Lock()
        server.state = dict(
            initiative_name="Task PR Tests",
            project=project,
            issues=[issue, other, remote],
            comments=[],
            offline=False,
            state_writes=0,
            requests=0,
            unexpected=[],
        )
        env["HTTPS_PROXY"] = f"http://127.0.0.1:{server.server_port}"
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            _exercise(fixture, env, server, sys.argv[2])
        finally:
            server.shutdown()
            thread.join()
    print(f"public {sys.argv[2]} reconnect: retained identities and independent propagation")


if __name__ == "__main__":
    main()
