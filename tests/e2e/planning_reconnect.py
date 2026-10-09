"""Public work-watch and Flow reconnect against a disposable Linear HTTPS peer."""

import copy
import fcntl
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
        if state.get("associations"):
            if "mutation DeliverTaskField" in query or "mutation DeliverProjectField" in query:
                target = project if "DeliverProjectField" in query else issue
                target.update(variables["input"])
                state["field_writes"].append((variables["id"], variables["input"]))
                state["revision"] += 1
                target["updatedAt"] = f"2026-10-09T12:00:{state['revision']:02d}Z"
                return {"errors": [{"message": "lost field response"}]}
        if state.get("exports"):
            exports = state["exports"]
            if (
                "query ListInitiativeProjects" in query
                and variables["initiativeId"] == "initiative-peer"
            ):
                # Creation recovery must not depend on complete-list acquisition.
                return {"data": {"initiative": {"projects": _page([])}}}
            if "query ListProjectIssues" in query and variables["projectId"] != project["id"]:
                return {"data": {"project": {"issues": _page([])}}}
            if "mutation DeliverProjectCreation" in query:
                exports["project_writes"] += 1
                value = variables["input"]
                exports["project"] = {
                    **copy.deepcopy(project),
                    "id": value["id"],
                    "name": value["name"],
                    "description": value["description"],
                    "content": value["content"],
                    "status": {"type": value["statusId"]},
                    "initiatives": _page([]),
                }
                return {"errors": [{"message": "lost project creation response"}]}
            if "mutation DeliverProjectAttachment" in query:
                exports["link_writes"] += 1
                exports["initiative"] = variables["input"]["initiativeId"]
                return {"errors": [{"message": "lost attachment response"}]}
            if "mutation DeliverTaskCreation" in query:
                exports["task_writes"] += 1
                value = variables["input"]
                exports["issue"] = {
                    **copy.deepcopy(state["issues"][0]),
                    "id": value["id"],
                    "identifier": "PEER-1",
                    "title": value["title"],
                    "description": value["description"],
                    "project": exports["project"],
                }
                return {"errors": [{"message": "lost task creation response"}]}
            if "query ProjectStatuses" in query:
                return {
                    "data": {
                        "projectStatuses": _page(
                            [
                                {"id": "planned", "type": "planned", "teamId": None, "position": 0},
                                {"id": "started", "type": "started", "teamId": None, "position": 1},
                            ]
                        )
                    }
                }
            if "query FindProject" in query:
                candidate = exports.get("project")
                nodes = (
                    [candidate]
                    if candidate
                    and exports["project_visible"]
                    and candidate["id"] == variables["id"]
                    else []
                )
                return {"data": {"projects": _page(nodes)}}
            if "query FindExportIssue" in query:
                candidate = exports.get("issue")
                nodes = (
                    [candidate]
                    if candidate and exports["task_visible"] and candidate["id"] == variables["id"]
                    else []
                )
                return {"data": {"issues": _page(nodes)}}
            if "mutation DeliverProjectField" in query:
                return {"errors": [{"message": "unrelated summary write rejected"}]}
            if (
                "query IssueOwnership" in query
                or "query IssueObservation" in query
                or "query IssueTeam" in query
            ):
                candidate = exports.get("issue")
                if candidate and candidate["id"] == variables["id"]:
                    if not exports["task_visible"]:
                        return {"errors": [{"message": "readback withheld"}]}
                    return {"data": {"issue": {**candidate, "comments": _page([])}}}
            if "query ProjectOwnership" in query:
                candidate = exports.get("project")
                if candidate and candidate["id"] == variables["id"]:
                    return {"data": {"project": candidate}}
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
        elif "query IssueTrash" in query:
            data = {"issue": {**issue, "trashed": False}}
        elif "mutation DeliverTaskDeletion" in query:
            state["delete_writes"] += 1
            data = {"issueDelete": {"success": True}}
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


def _execution_rows(db: sqlite3.Connection, processes: tuple[str, ...]) -> dict[str, list[tuple]]:
    rows = {
        table: db.execute(f"SELECT * FROM {table} ORDER BY rowid").fetchall()
        for table in [
            "agent_sessions",
            "task_workflows",
            "task_workflow_moves",
            "task_prs",
            "work_placements",
            "project_transitions",
        ]
    }
    # CLI inspection adds Processes; compare the retained identities, not their count.
    rows["processes"] = db.execute(
        "SELECT * FROM processes WHERE lfid IN (SELECT value FROM json_each(?)) ORDER BY rowid",
        (json.dumps(processes),),
    ).fetchall()
    return rows


def _assert_execution_unchanged(
    db: sqlite3.Connection, before: dict[str, list[tuple]], processes: tuple[str, ...]
) -> None:
    after = _execution_rows(db, processes)
    assert after == before, {k: (before[k], v) for k, v in after.items() if before[k] != v}


def _exercise_exports(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    root, repo = Path(fixture["home"]), Path(fixture["repo"])
    peer = fixture["peer"]
    db = sqlite3.connect(root / "loopflow.db", timeout=5)

    def run(*args: str) -> str:
        result = subprocess.run(
            [fixture["lf"], *args], cwd=repo, env=env, capture_output=True, text=True, timeout=30
        )
        assert result.returncode == 0, result.stderr
        return result.stdout

    def acknowledged(kind: str, identity: str) -> bool:
        return db.execute(
            "SELECT export_acknowledged FROM planning_creations WHERE kind=? AND origin_id=?",
            (kind, identity),
        ).fetchone() == (1,)

    def uncertain(kind: str, identity: str) -> bool:
        command = ("project", "workflow", "show") if kind == "project" else ("task", "status")
        changes = json.loads(run(*command, identity, "--json"))["sync"]["changes"]
        return any(
            c["field"] == "creation" and c["id"] == identity and c["state"] == "uncertain"
            for c in changes
        )

    wave_name = db.execute("SELECT name FROM waves WHERE id=?", (peer["wave"],)).fetchone()[0]
    run("wave", "edit", wave_name, "--goal", str(repo / "wave" / wave_name / "GOAL.md"))
    before = _execution_rows(db, ())
    with server.lock:
        server.state["exports"] = dict(
            project_writes=0,
            task_writes=0,
            link_writes=0,
            project_visible=False,
            task_visible=False,
        )
        exports = server.state["exports"]
    watch = Watch(fixture, env)
    try:
        watch.scope(str(repo))
        _await(lambda: exports["project_writes"] == 1, "unprepared peer Project was not exported")
        # Simulate a mapping-only peer acquisition after the response was lost.
        # This grants no acknowledgement and must not hide recovery or status.
        with server.lock:
            project_id = exports["project"]["id"]
        db.execute(
            "UPDATE projects SET external_project_id=? WHERE id=?", (project_id, peer["project"])
        )
        db.commit()
        assert uncertain("project", peer["project"])
        with server.lock:
            exports["project_visible"] = True
        _await(lambda: exports["link_writes"] == 1, "mapped Project bypassed attachment recovery")
        assert not acknowledged("project", peer["project"])
        assert uncertain("project", peer["project"])
        run("project", "edit", peer["project"], "--summary", "Later peer summary")
        with server.lock:
            exports["project"]["initiatives"] = _page([{"id": exports["initiative"]}])
        _await(
            lambda: acknowledged("project", peer["project"]),
            "attachment readback did not settle",
        )
        _await(lambda: exports["task_writes"] == 1, "unprepared peer Task was not exported")
        with server.lock:
            issue_id = exports["issue"]["id"]
        db.execute("UPDATE tasks SET external_issue_id=? WHERE id=?", (issue_id, peer["task"]))
        db.commit()
        assert uncertain("task", peer["task"])
        run("task", "edit", peer["task"], "--title", "Later peer title")
        with server.lock:
            exports["task_visible"] = True
        _await(
            lambda: acknowledged("task", peer["task"]),
            "mapped Task bypassed creation readback",
        )
        assert not uncertain("task", peer["task"])
        assert not uncertain("project", peer["project"])
        for table, key, identity, field, value in [
            ("task_changes", "task_id", peer["task"], "name", "Later peer title"),
            ("project_changes", "project_id", peer["project"], "summary", "Later peer summary"),
        ]:
            assert (
                db.execute(
                    f"SELECT count(*) FROM {table} WHERE {key}=? AND field=? AND value_json=? "
                    "AND acknowledged=0 AND conflict_json IS NULL",
                    (identity, field, json.dumps(value)),
                ).fetchone()[0]
                == 1
            )
        _assert_execution_unchanged(db, before, ())
        # Reopening the real foreground connection reobserves settled receipts.
        watch.close()
        watch = Watch(fixture, env)
        watch.scope(str(repo))
        watch.frame(lambda f: f["part"] == "planning")
        _assert_execution_unchanged(db, before, ())
        with server.lock:
            assert [exports[k] for k in ["project_writes", "task_writes", "link_writes"]] == [
                1,
                1,
                1,
            ]
            assert not server.state["unexpected"], server.state["unexpected"]
    except Exception as error:
        pending = db.execute("SELECT kind,attempted,error FROM planning_exports").fetchall()
        raise AssertionError(
            f"{error}; pending creations: {pending}; unexpected: {server.state['unexpected']}"
        ) from error
    finally:
        watch.close()
        db.close()


def _exercise_effects(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    db = sqlite3.connect(Path(fixture["home"]) / "loopflow.db", timeout=5)

    processes = ("00000000-0000-4000-8000-000000000001",)
    before = _execution_rows(db, processes)
    checkout = db.execute("SELECT worktree FROM tasks WHERE id=?", (fixture["task"],)).fetchone()
    saved = subprocess.run(
        [fixture["lf"], "task", "delete", fixture["task"]],
        cwd=fixture["repo"],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert saved.returncode == 0, saved.stderr
    watch = Watch(fixture, env)
    try:
        watch.scope(fixture["repo"])
        _await(
            lambda: (
                db.execute(
                    "SELECT count(*) FROM task_changes WHERE task_id=? AND field='deleted' "
                    "AND attempted=0 AND error LIKE '%retained peer projection conflict%'",
                    (fixture["task"],),
                ).fetchone()
                == (1,)
            ),
            "public delivery did not reach the retained-effect boundary",
        )
        # Mutate an unrelated provider record after observing the deferred write.
        # Ongoing acquisition must still bring in its new body and a new comment.
        with server.lock:
            other = server.state["issues"][1]
            other["title"] = "Independent acquisition continues"
            other["updatedAt"] = "2026-10-08T13:00:00Z"
            comment = _comment("While deletion is held", other["id"])
            server.state["comments"].append(comment)
        _await(
            lambda: (
                db.execute(
                    "SELECT count(*) FROM tasks WHERE external_issue_id=? AND issue_title=?",
                    (other["id"], other["title"]),
                ).fetchone()
                == (1,)
            ),
            "retained effect blocked independent acquisition",
        )
        _await(
            lambda: (
                db.execute(
                    "SELECT count(*) FROM task_comments WHERE id=?",
                    (comment["id"],),
                ).fetchone()
                == (1,)
            ),
            "retained effect blocked comment acquisition",
        )
        assert db.execute(
            "SELECT count(*) FROM task_changes WHERE id=?", (fixture["effects"]["receipt"],)
        ).fetchone() == (0,)
        assert (
            db.execute(
                "SELECT count(*) FROM planning_peer_conflicts WHERE object_id=? AND active=1",
                (fixture["task"],),
            ).fetchone()[0]
            > 0
        )
        _assert_execution_unchanged(db, before, processes)
        assert (
            db.execute("SELECT worktree FROM tasks WHERE id=?", (fixture["task"],)).fetchone()
            == checkout
        )
        with server.lock:
            assert server.state["delete_writes"] == 0
            assert not server.state["unexpected"], server.state["unexpected"]
    finally:
        watch.close()
        db.close()


def _exercise_associations(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    repo = Path(fixture["repo"])
    peer = {**fixture, **fixture["peer"]}
    peer_env = {**env, "HOME": peer["home"], "LF_HOME": peer["home"]}
    sides = [(fixture, env), (peer, peer_env)]
    databases = [sqlite3.connect(Path(f["home"]) / "loopflow.db", timeout=5) for f, _ in sides]
    processes = ("00000000-0000-4000-8000-000000000001",)

    def run(side: int, *args: str) -> str:
        result = subprocess.run(
            [fixture["lf"], *args],
            cwd=repo,
            env=sides[side][1],
            capture_output=True,
            text=True,
            timeout=45,
        )
        assert result.returncode == 0, (args, result.stdout, result.stderr)
        return result.stdout

    def exchange(side: int, predicate) -> None:
        watch = Watch(*sides[side])
        try:
            watch.scope(str(repo))
            try:
                _await(predicate, f"side {side} did not exchange")
            except AssertionError as error:
                reading = status(side)
                raise AssertionError(
                    {k: v for k, v in reading.items() if k != "records"}
                ) from error
        finally:
            watch.close()

    def status(side: int) -> dict:
        return json.loads(run(side, "planning", "status", "--json"))["destinations"][0]

    def title(side: int) -> str:
        return (
            databases[side]
            .execute("SELECT issue_title FROM tasks WHERE id=?", (sides[side][0]["task"],))
            .fetchone()[0]
        )

    def name(side: int) -> str:
        identity = fixture["local_project"] if side == 0 else peer["project"]
        return (
            databases[side]
            .execute("SELECT project_name FROM projects WHERE id=?", (identity,))
            .fetchone()[0]
        )

    subprocess.run(["git", "remote", "add", "plans", fixture["remote"]], cwd=repo, check=True)
    with server.lock:
        server.state.update(associations=True, field_writes=[], revision=0)
        server.state["issues"] = server.state["issues"][:1]
    for side in range(2):
        if not (side == 1 and fixture["private"]):
            run(side, "repo", "refresh", "--all")
        destination = run(
            side, "planning", "connect", "--remote", "plans", "--shared", "composition"
        ).strip()
        if not (side == 1 and fixture["private"]):
            run(side, "planning", "select", destination, "--wave", fixture["wave"])
    before = [_execution_rows(db, processes) for db in databases]
    if fixture["private"]:
        with server.lock:
            server.state["offline"] = True
        run(1, "task", "edit", peer["task"], "--title", "Private associated title")
        run(1, "project", "edit", peer["project"], "--name", "Private associated project")
        exchange(0, lambda: status(0)["publication_state"] == "confirmed")
        exchange(1, lambda: status(1)["imported_revision"] is not None)
        for incoming, local, provider in [
            (fixture["task"], peer["task"], fixture["issue"]),
            (fixture["local_project"], peer["project"], fixture["project"]),
        ]:
            for _ in range(2):
                run(1, "planning", "associate", incoming, "--with", local, "--linear", provider)
        revision = status(1)["imported_revision"]
        # A later public source save supplies a distinct import checkpoint.
        run(0, "task", "edit", fixture["task"], "--title", "Shared later title")
        exchange(1, lambda: status(1)["imported_revision"] != revision)
        reading = status(1)
        assert reading["conflicts"]
        assert title(1) == "Private associated title"
        assert name(1) == "Private associated project"
        document = subprocess.check_output(
            [
                "git",
                "--git-dir",
                fixture["remote"],
                "show",
                "refs/loopflow/planning/shared/composition:planning.json",
            ],
            text=True,
        )
        assert "Private associated title" not in document
        assert "Private associated project" not in document
        assert peer["task"] not in document and peer["project"] not in document
        assert not server.state["field_writes"]
        for side, db in enumerate(databases):
            _assert_execution_unchanged(db, before[side], processes)
            db.close()
        return
    # An attempted local edit is losing evidence, not permission to repeat it.
    with server.lock:
        server.state["offline"] = True
    run(1, "task", "edit", peer["task"], "--title", "Uncertain losing title")
    run(1, "project", "edit", peer["project"], "--name", "Uncertain losing project")
    losing = []
    for table in ["task_changes", "project_changes"]:
        databases[1].execute(
            f"UPDATE {table} SET attempted=1,error='lost reply' WHERE acknowledged=0"
        )
        losing.extend(
            (table, *row)
            for row in databases[1].execute(
                f"SELECT id,value_json,base_json FROM {table} WHERE acknowledged=0"
            )
        )
    databases[1].commit()
    with server.lock:
        server.state["offline"] = False
        server.state["issues"][0]["title"] = "Observed peer title"
        server.state["issues"][0]["updatedAt"] = "2026-10-09T12:00:00Z"
        server.state["project"]["name"] = "Observed peer project"
        server.state["project"]["updatedAt"] = "2026-10-09T12:00:00Z"
    run(0, "repo", "refresh", "--all")
    # The first public fetch retains duplicate IDs without guessing correspondence.
    exchange(
        0,
        lambda: (
            status(0)["publication_state"] == "confirmed" and status(0)["pending_local"] is False
        ),
    )
    for side in [1, 0]:
        exchange(side, lambda side=side: bool(status(side)["conflicts"]))
        for incoming, local, provider in [
            (
                fixture["task"] if side else peer["task"],
                peer["task"] if side else fixture["task"],
                fixture["issue"],
            ),
            (
                fixture["local_project"] if side else peer["project"],
                peer["project"] if side else fixture["local_project"],
                fixture["project"],
            ),
        ]:
            for _ in range(2):
                run(side, "planning", "associate", incoming, "--with", local, "--linear", provider)
        exchange(side, lambda side=side: not status(side)["conflicts"])
    for side in range(2):
        exchange(
            side,
            lambda side=side: (
                title(side) == "Observed peer title" and name(side) == "Observed peer project"
            ),
        )
    earlier = status(0)["publication_revision"]
    # A local save after a peer observation must carry that causal baseline back.
    with server.lock:
        server.state["offline"] = True
    run(1, "task", "edit", peer["task"], "--title", "After peer observation")
    run(1, "project", "edit", peer["project"], "--name", "After peer project")
    exchange(0, lambda: title(0) == "After peer observation" and name(0) == "After peer project")
    for side, db in enumerate(databases):
        for table, owner_column, owner, value in [
            ("task_changes", "task_id", sides[side][0]["task"], "After peer observation"),
            (
                "project_changes",
                "project_id",
                fixture["local_project"] if side == 0 else peer["project"],
                "After peer project",
            ),
        ]:
            rows = db.execute(
                f"SELECT base_json FROM {table} WHERE {owner_column}=? AND value_json=? "
                "AND acknowledged=0 AND conflict_json IS NULL",
                (owner, json.dumps(value)),
            ).fetchall()
            assert rows and all(
                json.loads(row[0])["revision"] == "2026-10-09T12:00:00Z" for row in rows
            ), rows
    for table, identity, value, baseline in losing:
        assert databases[1].execute(
            "SELECT attempted,acknowledged,value_json,base_json,conflict_json IS NOT NULL "
            f"FROM {table} WHERE id=?",
            (identity,),
        ).fetchone() == (1, 0, value, baseline, 1)
    with server.lock:
        server.state["offline"] = False
    exchange(
        0,
        lambda: (
            server.state["issues"][0]["title"] == "After peer observation"
            and server.state["project"]["name"] == "After peer project"
        ),
    )
    for side in [1, 0, 1]:
        exchange(
            side,
            lambda side=side: (
                not status(side)["conflicts"] and status(side)["pending_local"] is False
            ),
        )
        assert (
            json.loads(run(side, "task", "status", fixture["task"], "--json"))["execution"][
                "task_id"
            ]
            == sides[side][0]["task"]
        )
        assert (
            json.loads(run(side, "task", "status", peer["task"], "--json"))["execution"]["task_id"]
            == sides[side][0]["task"]
        )
        assert json.loads(
            run(side, "project", "workflow", "show", fixture["local_project"], "--json")
        ) == json.loads(run(side, "project", "workflow", "show", peer["project"], "--json"))
        _assert_execution_unchanged(databases[side], before[side], processes)
    with server.lock:
        assert len(server.state["field_writes"]) == 2, server.state["field_writes"]
        assert not server.state["unexpected"], server.state["unexpected"]
    # Acquire older then repeated documents from new, fast-forward Git commits.
    # Hold only publication so the ordinary acquisition path cannot replace the
    # checkpoint before it is inspected; provider readback remains independent.
    reference = "refs/loopflow/planning/shared/composition"
    git_env = {
        **env,
        "GIT_AUTHOR_NAME": "Fixture",
        "GIT_AUTHOR_EMAIL": "fixture@example.test",
        "GIT_COMMITTER_NAME": "Fixture",
        "GIT_COMMITTER_EMAIL": "fixture@example.test",
    }

    def git(*args: str) -> str:
        return subprocess.check_output(
            ["git", "--git-dir", fixture["remote"], *args], env=git_env, text=True
        ).strip()

    for side in [1, 0, 1]:
        current = git("rev-parse", reference)
        revision = git(
            "commit-tree",
            git("rev-parse", f"{earlier}^{{tree}}"),
            "-p",
            current,
            "-m",
            "Retained earlier planning document",
        )
        git("update-ref", reference, revision, current)
        lock = Path(sides[side][0]["home"]) / "locks/planning-peers" / f"{status(side)['id']}.lock"
        with lock.open("a") as handle:
            fcntl.flock(handle, fcntl.LOCK_EX)
            exchange(side, lambda side=side: status(side)["imported_revision"] == revision)
            assert title(side) == "After peer observation"
            assert name(side) == "After peer project"
        _assert_execution_unchanged(databases[side], before[side], processes)
    with server.lock:
        assert len(server.state["field_writes"]) == 2, server.state["field_writes"]
    for side, db in enumerate(databases):
        _assert_execution_unchanged(db, before[side], processes)
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
            delete_writes=0,
            unexpected=[],
        )
        env["HTTPS_PROXY"] = f"http://127.0.0.1:{server.server_port}"
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            if sys.argv[2] in ("associations", "association-private"):
                _exercise_associations(fixture, env, server)
            elif sys.argv[2] == "exports":
                _exercise_exports(fixture, env, server)
            elif sys.argv[2] == "effects":
                _exercise_effects(fixture, env, server)
            else:
                _exercise(fixture, env, server, sys.argv[2])
        finally:
            server.shutdown()
            thread.join()
    print(f"public {sys.argv[2]} reconnect: retained identities and independent propagation")


if __name__ == "__main__":
    main()
