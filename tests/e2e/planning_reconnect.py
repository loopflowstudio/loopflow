"""Public work-watch and Flow reconnect against a disposable Linear HTTPS peer."""

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
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from linear_fixture import tls_context


def _page(nodes: list) -> dict:
    return {"nodes": nodes, "pageInfo": {"hasNextPage": False, "endCursor": None}}


def _comment(body: str, issue: str, identity: str | None = None) -> dict:
    return {
        "id": identity or str(uuid.uuid4()),
        "body": body,
        "issue": {"id": issue},
        "createdAt": "2026-10-08T12:00:00Z",
        "updatedAt": "2026-10-08T12:00:00Z",
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
        if state["offline"]:
            return {"errors": [{"message": "fixture offline"}]}
        issue = next((i for i in state["issues"] if i["id"] == variables.get("id")), None)
        project = state["project"]
        if "query ListTeams" in query:
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
                            "description": "",
                        }
                    ]
                )
            }
        elif "mutation UpdateInitiative" in query:
            state["initiative_name"] = variables["name"]
            data = {"initiativeUpdate": {"initiative": {"id": variables["id"]}}}
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
            issue["updatedAt"] = "2026-10-08T12:00:02Z"
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
    def __init__(self, fixture: dict, env: dict):
        self.process = subprocess.Popen(
            [fixture["lf"], "monitor", "work", "--watch", "--json"],
            cwd=fixture["repo"],
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        self.frames = queue.Queue()
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        self.serial = 0

    def _read(self) -> None:
        for line in self.process.stdout:
            self.frames.put(json.loads(line))

    def scope(self, repo: str | None, wave: str | None = None, task: str | None = None) -> None:
        self.serial += 1
        request = dict(
            action="scope",
            id=self.serial,
            repo=repo,
            wave=wave,
            task=task,
            headless=True,
            activity=None,
        )
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        # Wait for the public scope acknowledgement, not a timing guess.
        self.frames.put(self.frame(lambda f: f["answers"] == self.serial))

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

    # Acquire a real baseline before the outage, not an invented provider revision.
    run("repo", "refresh", "--all")
    remote_id = db.execute(
        "SELECT id FROM tasks WHERE external_issue_id=?", (server.state["issues"][2]["id"],)
    ).fetchone()[0]
    local_id = db.execute("SELECT id FROM tasks WHERE issue_identifier='INF-124'").fetchone()[0]
    prs = db.execute("SELECT id,task_id,branch,base_commit FROM task_prs").fetchall()
    watch = None
    flow = None
    flow_log = None
    try:
        with server.lock:
            server.state["offline"] = True
        if mode == "watch":
            watch = Watch(fixture, env)
            watch.scope(str(repo))  # No Task or Wave selected.
        else:
            # Flow admission requires a managed login, even with a contained provider.
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
            (repo / ".lf/flows").mkdir(exist_ok=True)
            (repo / ".lf/skills").mkdir(exist_ok=True)
            (repo / ".lf/flows/reconnect.yaml").write_text("- reconnect-proof\n")
            (repo / ".lf/skills/reconnect-proof.md").write_text(
                "---\nagent: claude\n---\nWait for the fixture.\n"
            )
            flow_log = (root / "flow.log").open("w+")
            flow = subprocess.Popen(
                [fixture["lf"], "-b", "--task", "INF-123", "run", "reconnect"],
                cwd=repo,
                env=env,
                stdout=flow_log,
                stderr=flow_log,
            )
            _await(
                lambda: (root / "provider-started").exists() or flow.poll() is not None,
                "Flow never launched its provider",
            )
            assert flow.poll() is None, (root / "flow.log").read_text()
        run("task", "edit", "INF-123", "--title", "Pending local title")
        run("task", "move", "INF-124", "end", "--reason", "Saved offline")
        saved = json.loads(run("task", "comment", "INF-124", "Saved offline", "--json"))
        comment_id = saved["pending_sync"][0]
        receipt = db.execute(
            "SELECT id FROM task_state_deliveries WHERE task_id=? ORDER BY seq DESC LIMIT 1",
            (local_id,),
        ).fetchone()[0]
        workflow = db.execute("SELECT * FROM task_workflows ORDER BY task_id").fetchall()
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
            state["issues"][2]["completedAt"] = "2026-10-08T12:00:01Z"
            state["issues"][2]["updatedAt"] = "2026-10-08T12:00:01Z"
            added = copy.deepcopy(state["issues"][0])
            added.update(id=str(uuid.uuid4()), identifier="INF-126", title="Added while offline")
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
                    "SELECT json_extract(body,'$.completed') FROM pm_items WHERE id=?",
                    (state["issues"][2]["id"],),
                ).fetchone()
                == (1,)
            )

        _await(caught_up, "active connection did not catch up without a turn or refresh")
        assert db.execute("SELECT * FROM task_workflows ORDER BY task_id").fetchall() == workflow
        assert db.execute("SELECT id,task_id,branch,base_commit FROM task_prs").fetchall() == prs
        assert db.execute("SELECT worktree FROM tasks WHERE id=?", (remote_id,)).fetchone() == (
            None,
        )
        assert db.execute(
            "SELECT count(*) FROM task_changes WHERE task_id=? AND field='name' "
            "AND conflict_json IS NULL AND acknowledged=0",
            (fixture["task"],),
        ).fetchone() == (1,)
        with server.lock:
            assert server.state["state_writes"] == 1
            assert len([c for c in server.state["comments"] if c["id"] == comment_id]) == 1
            assert not server.state["unexpected"], server.state["unexpected"]
        if watch:
            watch.frame(lambda f: f["part"] == "planning" and "INF-126" in json.dumps(f))
            # Task selection only changes projection. Leaving it must not stop sync.
            watch.scope(str(repo), task="INF-125")
            watch.frame(lambda f: f["part"] == "task" and incoming["id"] in json.dumps(f))
            watch.scope(str(repo), wave=fixture["wave"])
            later = _comment("After leaving Task", state["issues"][2]["id"])
            with server.lock:
                state["comments"].append(later)
            _await(
                lambda: (
                    db.execute(
                        "SELECT count(*) FROM task_comments WHERE id=?", (later["id"],)
                    ).fetchone()
                    == (1,)
                ),
                "leaving Task stopped acquisition",
            )
            watch.close()
            watch = None
            # No active connection: a local save remains local. A reopened repository
            # connection delivers it and acquires changes made during the gap.
            gap = json.loads(
                run("task", "comment", "INF-124", "Saved with Desktop closed", "--json")
            )["pending_sync"][0]
            with server.lock:
                assert not any(c["id"] == gap for c in state["comments"])
            watch = Watch(fixture, env)
            watch.scope(str(repo), wave=fixture["wave"])
            _await(
                lambda: (
                    db.execute(
                        "SELECT acknowledged FROM task_comment_deliveries WHERE comment_id=?",
                        (gap,),
                    ).fetchone()
                    == (1,)
                ),
                "reopened Wave view did not deliver",
            )
            watch.scope(None)  # Connection remains open, but no repository is selected.
            detached = json.loads(
                run("task", "comment", "INF-124", "No repository selected", "--json")
            )["pending_sync"][0]
            time.sleep(2)
            assert db.execute(
                "SELECT acknowledged FROM task_comment_deliveries WHERE comment_id=?", (detached,)
            ).fetchone() == (0,)
            watch.scope(str(repo))
            _await(
                lambda: (
                    db.execute(
                        "SELECT acknowledged FROM task_comment_deliveries WHERE comment_id=?",
                        (detached,),
                    ).fetchone()
                    == (1,)
                ),
                "repository reselection did not deliver",
            )
        else:
            assert flow.poll() is None
            (root / "provider-stop").touch()
            assert flow.wait(timeout=20) == 0, (root / "flow.log").read_text()
            assert db.execute("SELECT count(*) FROM flow_process_steps").fetchone()[0] == 1
    finally:
        (root / "provider-stop").touch()
        if watch:
            watch.close()
        if flow and flow.poll() is None:
            try:
                flow.wait(timeout=20)
            except subprocess.TimeoutExpired:
                flow.kill()
                flow.wait()
        if flow_log:
            flow_log.close()
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
        "content": "workflow: feature",
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
    provider.write_text("""#!/bin/sh
: > "$HOME/provider-started"
i=0
while [ ! -e "$HOME/provider-stop" ] && [ "$i" -lt 900 ]; do
  sleep 0.1
  i=$((i+1))
done
printf '%s\\n' '{"type":"result","subtype":"success","result":"Done"}'
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
