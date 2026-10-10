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
from functools import partial
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
        if state.get("ordering"):
            if "query IssueOwnership" in query and state["order_writes"]:
                moved = state["order_writes"][0]["id"]
                if variables["id"] == moved and state["list_mode"] == "partial":
                    state["moved_detail_reads"] += 1
            if "query ProjectOwnership" in query:
                project = next(p for p in state["projects"] if p["id"] == variables["id"])
            if "query ListInitiativeProjects" in query:
                return {"data": {"initiative": {"projects": _page(state["projects"])}}}
            if "query IssueTrash" in query:
                return {"data": {"issue": {**issue, "trashed": issue.get("trashed", False)}}}
            if "mutation DeliverTaskDeletion" in query:
                state["delete_writes"] += 1
                issue["trashed"] = True
                issue["updatedAt"] = "2026-10-09T12:00:00Z"
                return {"errors": [{"message": "lost deletion response"}]}
            if "query ListProjectIssues" in query:
                state["list_reads"][state["list_mode"]] += 1
                nodes = [
                    i
                    for i in state["issues"]
                    if i["project"]["id"] == variables["projectId"] and not i.get("trashed", False)
                ]
                if state["list_mode"] == "partial" and variables["projectId"] == project["id"]:
                    nodes = nodes[:-1]
                return {"data": {"project": {"issues": _page(nodes)}}}
            if "mutation DeliverTaskField" in query and "prioritySortOrder" in variables["input"]:
                state["order_writes"].append(copy.deepcopy(variables))
                issue.update(variables["input"])
                # Linear can change rank without changing the entity revision.
                # Lose the reply and withhold a member on subsequent list reads.
                if len(state["order_writes"]) == 1:
                    state["list_mode"] = "partial"
                return {"errors": [{"message": "lost order response"}]}
        if state.get("associated_creations"):
            if any(
                name in query
                for name in (
                    "mutation DeliverProjectCreation",
                    "mutation DeliverProjectAttachment",
                    "mutation DeliverTaskCreation",
                )
            ):
                state["creation_writes"].append(variables)
                return {"errors": [{"message": "retained creation must not replay"}]}
            if "query FindExportIssue" in query:
                return {"data": {"issues": _page([issue] if issue else [])}}
            if "query FindProject" in query:
                return {
                    "data": {
                        "projects": _page([project] if variables["id"] == project["id"] else [])
                    }
                }
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
                value = variables["input"]
                exports.setdefault("writes", []).append(("project", value["id"]))
                exports["project_writes"] += 1
                if exports.get("project", {}).get("id") == value["id"]:
                    return {"errors": [{"message": "project id already exists"}]}
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
                identity = variables["input"]["id"]
                exports.setdefault("writes", []).append(("link", identity))
                exports["link_writes"] += 1
                if identity in exports.setdefault("links", []):
                    return {"errors": [{"message": "attachment id already exists"}]}
                exports["links"].append(identity)
                exports["initiative"] = variables["input"]["initiativeId"]
                return {"errors": [{"message": "lost attachment response"}]}
            if "mutation DeliverTaskCreation" in query:
                value = variables["input"]
                exports.setdefault("writes", []).append(("task", value["id"]))
                exports["task_writes"] += 1
                if exports.get("issue", {}).get("id") == value["id"]:
                    return {"errors": [{"message": "issue id already exists"}]}
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


def _run(fixture: dict, env: dict, *args: str, timeout: int = 30) -> str:
    result = subprocess.run(
        [fixture["lf"], *args],
        cwd=fixture["repo"],
        env=env,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    assert result.returncode == 0, (args, result.stdout, result.stderr)
    return result.stdout


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

    run = partial(_run, fixture, env)

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
            "session_events",
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

    run = partial(_run, fixture, env)

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


def _peer_sides(fixture: dict, env: dict) -> list[tuple[dict, dict]]:
    peer = {**fixture, **fixture["peer"]}
    peer_env = {
        **env,
        "HOME": peer["home"],
        "LF_HOME": peer["home"],
        "CLAUDE_CONFIG_DIR": str(Path(peer["home"]) / "claude"),
        "CODEX_HOME": str(Path(peer["home"]) / "codex"),
    }
    return [(fixture, env), (peer, peer_env)]


def _planning_status(fixture: dict, env: dict) -> dict:
    return json.loads(_run(fixture, env, "planning", "status", "--json", timeout=45))[
        "destinations"
    ][0]


def _exchange(fixture: dict, env: dict, predicate, *, wait_for_publication: bool = False) -> None:
    def published() -> bool:
        reading = _planning_status(fixture, env)
        return reading["publication_state"] == "confirmed" and reading["pending_local"] is False

    watch = Watch(fixture, env)
    try:
        watch.scope(fixture["repo"])
        _await(predicate, "peer did not exchange")
        if wait_for_publication:
            _await(published, "peer did not publish receipts")
    except AssertionError as error:
        reading = _planning_status(fixture, env)
        raise AssertionError(
            {"home": fixture["home"], **{k: v for k, v in reading.items() if k != "records"}}
        ) from error
    finally:
        watch.close()


def _exercise_ordering(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    sides = _peer_sides(fixture, env)
    databases = [sqlite3.connect(Path(f["home"]) / "loopflow.db", timeout=5) for f, _ in sides]
    processes = ("00000000-0000-4000-8000-000000000001",)
    reference = "refs/loopflow/planning/shared/ordering"

    def run(side: int, *args: str) -> str:
        return _run(*sides[side], *args, timeout=45)

    def exchange(side: int, predicate=lambda: True) -> None:
        _exchange(*sides[side], predicate, wait_for_publication=True)

    def receipt(side: int, identity: str) -> tuple | None:
        return (
            databases[side]
            .execute(
                "SELECT value_json,base_json,order_effects_json,attempted,acknowledged,"
                "conflict_json "
                "FROM project_changes WHERE id=?",
                (identity,),
            )
            .fetchone()
        )

    def removal(side: int) -> tuple | None:
        return (
            databases[side]
            .execute(
                "SELECT id,value_json,base_json,attempted,acknowledged,conflict_json,"
                "deletion_saved_at,acknowledged_revision FROM task_changes "
                "WHERE field='deleted'",  # Only the independent Task is removed.
            )
            .fetchone()
        )

    subprocess.run(
        ["git", "remote", "add", "plans", fixture["remote"]], cwd=fixture["repo"], check=True
    )
    with server.lock:
        fourth = copy.deepcopy(server.state["issues"][0])
        fourth.update(id=str(uuid.uuid4()), identifier="INF-126", title="Fourth ordered Task")
        server.state["issues"].append(fourth)
        independent_project = copy.deepcopy(server.state["project"])
        independent_project.update(
            id=str(uuid.uuid4()), name="Independent removal", status={"type": "planned"}
        )
        independent = copy.deepcopy(fourth)
        independent.update(id=str(uuid.uuid4()), identifier="INF-127", project=independent_project)
        server.state["projects"] = [server.state["project"], independent_project]
        server.state["issues"].append(independent)
        for index, issue in enumerate(server.state["issues"]):
            issue.update(prioritySortOrder=0, sortOrder=index * 10)
        server.state.update(
            ordering=True,
            list_mode="complete",
            order_writes=[],
            moved_detail_reads=0,
            list_reads={"complete": 0, "partial": 0},
        )
    run(0, "repo", "refresh", "--all")
    for side in range(2):
        destination = run(
            side, "planning", "connect", "--remote", "plans", "--shared", "ordering"
        ).strip()
        if side == 0:
            run(side, "planning", "select", destination, "--wave", fixture["wave"])
    with server.lock:
        server.state["offline"] = True
    exchange(0)
    exchange(1, lambda: databases[1].execute("SELECT count(*) FROM tasks").fetchone() == (5,))
    # Wave documents remain local. Explicitly ingest the checked-out definition
    # before expecting this machine to acquire/deliver through its Linear mapping.
    run(
        1,
        "wave",
        "edit",
        "task-pr-tests",
        "--goal",
        str(Path(fixture["repo"]) / "wave/task-pr-tests/GOAL.md"),
    )
    # Populate receiver execution only after cold Git acquisition. This is fixture
    # history, not transported execution. Neither exchange may change these rows.
    for table in [
        "processes",
        "agent_sessions",
        "task_workflows",
        "task_workflow_moves",
        "task_prs",
    ]:
        query = f"SELECT * FROM {table}"
        if table == "processes":
            query += " WHERE lfid='00000000-0000-4000-8000-000000000001'"
        rows = databases[0].execute(query).fetchall()
        for row in rows:
            databases[1].execute(f"INSERT INTO {table} VALUES({','.join('?' for _ in row)})", row)
    databases[1].commit()
    before = [_execution_rows(db, processes) for db in databases]
    run(0, "task", "edit", "INF-126", "--rank", "0")
    run(0, "task", "edit", "INF-125", "--rank", "0")
    with server.lock:
        server.state["offline"] = False
    watch = Watch(*sides[0])
    try:
        watch.scope(fixture["repo"])
        _await(lambda: bool(server.state["order_writes"]), "no public order attempt")
        _await(lambda: server.state["list_reads"]["partial"] > 0, "no incomplete readback")
    finally:
        watch.close()
    attempted = (
        databases[0]
        .execute("SELECT id FROM project_changes WHERE field='task_order' AND attempted=1")
        .fetchone()[0]
    )
    captured = receipt(0, attempted)
    assert captured[3:5] == (1, 0), captured
    assert not json.loads(captured[2])[-1]["settled"], captured
    # Remove unrelated planning through the CLI while the first order move is
    # uncertain. The provider commits deletion but loses its response; trash
    # readback, not absence from a list, must confirm the retained receipt.
    run(0, "task", "delete", "INF-127")
    exchange(0, lambda: removal(0) is not None and removal(0)[4] == 1)
    negative = removal(0)
    with server.lock:
        server.state["offline"] = True
    exchange(0)
    earlier = _planning_status(*sides[0])["publication_revision"]
    exchange(1, lambda: removal(1) == negative and receipt(1, attempted) == captured)
    run(1, "task", "edit", "INF-124", "--rank", "0")
    later = (
        databases[1]
        .execute(
            "SELECT id,value_json FROM project_changes WHERE field='task_order' "
            "ORDER BY seq DESC LIMIT 1"
        )
        .fetchone()
    )
    assert later[0] != attempted
    run(1, "task", "edit", "INF-125", "--title", "Retained independent save")
    saved = json.loads(
        run(1, "task", "comment", "INF-124", "Comment during partial order", "--json")
    )
    comment = saved["pending_sync"][0]
    with server.lock:
        server.state["offline"] = False
        incoming = _comment("Incoming during partial order", server.state["issues"][1]["id"])
        server.state["comments"].append(incoming)
        partial_reads = server.state["list_reads"]["partial"]
    watch = Watch(*sides[1])
    try:
        watch.scope(fixture["repo"])
        _await(
            lambda: (
                server.state["list_reads"]["partial"] > partial_reads
                and server.state["moved_detail_reads"] > 0
                and databases[1]
                .execute(
                    "SELECT acknowledged FROM task_comment_deliveries WHERE comment_id=?",
                    (comment,),
                )
                .fetchone()
                == (1,)
                and databases[1]
                .execute("SELECT count(*) FROM task_comments WHERE id=?", (incoming["id"],))
                .fetchone()
                == (1,)
            ),
            "partial ordering blocked independent comments",
        )
        assert receipt(1, attempted) == captured
        assert len(server.state["order_writes"]) == 1
        assert removal(1) == negative
    except AssertionError as error:
        raise AssertionError(
            {
                "comment": databases[1]
                .execute("SELECT * FROM task_comment_deliveries WHERE comment_id=?", (comment,))
                .fetchall(),
                "incoming": databases[1].execute("SELECT id,body FROM task_comments").fetchall(),
                "lists": (partial_reads, server.state["list_reads"]),
                "details": server.state["moved_detail_reads"],
                "delivery": databases[1]
                .execute("SELECT id,field,error FROM project_changes")
                .fetchall(),
                "unexpected": server.state["unexpected"],
                "conflicts": _planning_status(*sides[1])["conflicts"],
            }
        ) from error
    finally:
        watch.close()
    # Detail carries the new sort keys at the unchanged entity revision, but
    # only a complete Project list may settle the order move.
    assert receipt(1, attempted) == captured
    with server.lock:
        server.state["list_mode"] = "complete"
    exchange(1, lambda: receipt(1, later[0])[4] == 1)
    with server.lock:
        server.state["offline"] = True
    exchange(0, lambda: receipt(0, later[0]) == receipt(1, later[0]))
    assert receipt(0, attempted) == receipt(1, attempted)
    assert receipt(0, later[0])[0] == later[1]
    for side in range(2):
        assert databases[side].execute(
            "SELECT issue_title FROM tasks WHERE issue_identifier='INF-125'"
        ).fetchone() == ("Retained independent save",)
        assert removal(side) == negative
    # Earlier Git documents may arrive again without erasing detail, receipts or
    # authored alternatives. Every effect retains its exact attempted input.
    settled = [receipt(1, identity) for identity in (attempted, later[0])]
    for side in [0, 1, 0]:
        replay = _replay_planning_document(fixture, env, reference, earlier)
        exchange(
            side, lambda side=side: _planning_status(*sides[side])["imported_revision"] == replay
        )
        assert [receipt(side, identity) for identity in (attempted, later[0])] == settled
        assert removal(side) == negative
    for side, db in enumerate(databases):
        _assert_execution_unchanged(db, before[side], processes)
        assert not (Path(sides[side][0]["home"]) / "provider-started").exists()
        db.close()
    with server.lock:
        assert not server.state["unexpected"], server.state["unexpected"]
        assert server.state["delete_writes"] == 1
        effects = [effect for row in settled for effect in json.loads(row[2])]
        assert len(server.state["order_writes"]) == len(effects)
        assert all(effect["settled"] for effect in effects)
        assert sorted(
            (w["id"], json.dumps(w["input"], sort_keys=True)) for w in server.state["order_writes"]
        ) == sorted((e["issue"], json.dumps(e["input"], sort_keys=True)) for e in effects)


def _exercise_creation_origins(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    sides = _peer_sides(fixture, env)
    peer = sides[1][0]
    databases = [sqlite3.connect(Path(f["home"]) / "loopflow.db", timeout=5) for f, _ in sides]
    processes = ("00000000-0000-4000-8000-000000000001",)

    def run(side: int, *args: str) -> str:
        return _run(*sides[side], *args, timeout=45)

    def receipt(side: int, kind: str) -> tuple | None:
        return (
            databases[side]
            .execute(
                "SELECT export_attempted,export_link_attempted,export_acknowledged,export_json "
                "FROM planning_creations WHERE kind=? AND origin_id=?",
                (kind, peer[kind]),
            )
            .fetchone()
        )

    def exchange(side: int, predicate) -> None:
        _exchange(*sides[side], predicate, wait_for_publication=True)

    with server.lock:
        server.state["exports"] = dict(
            project_writes=0,
            task_writes=0,
            link_writes=0,
            project_visible=False,
            task_visible=False,
        )
        exports = server.state["exports"]
    for side, (selected, _) in enumerate(sides):
        repo = Path(selected["repo"])
        subprocess.run(["git", "remote", "add", "plans", fixture["remote"]], cwd=repo, check=True)
        subprocess.run(
            ["git", "remote", "set-url", "origin", "https://github.com/loopflowstudio/fixture.git"],
            cwd=repo,
            check=True,
        )
        config = repo / ".lf" / "config.yaml"
        config.parent.mkdir(exist_ok=True)
        config.write_text("pm:\n  provider: linear\n  linear_team: team-task-pr-tests\n")
        goal = repo / "wave" / peer["wave_name"] / "GOAL.md"
        goal.parent.mkdir(parents=True, exist_ok=True)
        goal.write_text(
            f"---\nid: {peer['wave']}\npm:\n  linear_initiative: initiative-peer\n---\n"
            "Creation recovery\n"
        )
        destination = run(
            side, "planning", "connect", "--remote", "plans", "--shared", "origins"
        ).strip()
        if side == 1:
            run(side, "wave", "edit", peer["wave_name"], "--goal", str(goal))
            run(side, "planning", "select", destination, "--wave", peer["wave"])
    before = [_execution_rows(db, processes) for db in databases]
    assert (
        databases[0].execute("SELECT id FROM tasks WHERE id=?", (peer["task"],)).fetchone() is None
    )
    try:
        # Source creates; receiver attaches; source creates the Task. Each reply
        # is lost, so the opposite store must recover the original effect receipt.
        exchange(1, lambda: exports["project_writes"] == 1)
        captured_project = receipt(1, "project")[3]
        exchange(0, lambda: receipt(0, "project") is not None)
        assert receipt(0, "project")[:3] == (1, 0, 0)
        assert receipt(0, "project")[3] == captured_project
        # Imported planning is unplaced; local saved Wave definitions are an
        # explicit configuration operation, not inferred from files on disk.
        run(
            0,
            "wave",
            "edit",
            peer["wave_name"],
            "--goal",
            str(Path(fixture["repo"]) / "wave" / peer["wave_name"] / "GOAL.md"),
        )
        with server.lock:
            exports["project_visible"] = True
        exchange(0, lambda: exports["link_writes"] == 1)
        assert receipt(0, "project")[:3] == (1, 1, 0)
        run(0, "project", "edit", peer["project"], "--summary", "Saved after creation")
        exchange(1, lambda: receipt(1, "project")[:3] == (1, 1, 0))
        with server.lock:
            exports["project"]["initiatives"] = _page([{"id": exports["initiative"]}])
        exchange(1, lambda: exports["task_writes"] == 1)
        captured_task = receipt(1, "task")[3]
        exchange(0, lambda: receipt(0, "task") is not None and receipt(0, "task")[0] == 1)
        assert receipt(0, "task")[3] == captured_task
        run(0, "task", "edit", peer["task"], "--title", "Saved after task creation")
        with server.lock:
            exports["task_visible"] = True
        exchange(0, lambda: receipt(0, "task")[2] == 1)
        exchange(1, lambda: receipt(1, "task")[2] == 1)
        writes_after_settlement = list(exports["writes"])
        assert set(writes_after_settlement) == {
            ("project", json.loads(captured_project)["id"]),
            ("link", json.loads(captured_project)["link_id"]),
            ("task", json.loads(captured_task)["id"]),
        }
        for side, db in enumerate(databases):
            assert receipt(side, "project") == (1, 1, 1, captured_project)
            assert receipt(side, "task") == (1, 0, 1, captured_task)
            for table, owner, identity, field, value in [
                ("task_changes", "task_id", peer["task"], "name", "Saved after task creation"),
                (
                    "project_changes",
                    "project_id",
                    peer["project"],
                    "summary",
                    "Saved after creation",
                ),
            ]:
                assert db.execute(
                    f"SELECT count(*) FROM {table} WHERE {owner}=? AND field=? AND value_json=? "
                    "AND acknowledged=0 AND conflict_json IS NULL",
                    (identity, field, json.dumps(value)),
                ).fetchone() == (1,)
            assert db.execute(
                "SELECT issue_title FROM tasks WHERE id=?", (peer["task"],)
            ).fetchone() == ("Saved after task creation",)
            assert db.execute(
                "SELECT count(*) FROM tasks WHERE external_issue_id=?", (exports["issue"]["id"],)
            ).fetchone() == (1,)
            _assert_execution_unchanged(db, before[side], processes)
            # Reconnect after settlement; neither Git replay nor HTTPS readback
            # can repeat a creation/attachment or allocate receiver execution.
            exchange(side, lambda: receipt(side, "task")[2] == 1)
            _assert_execution_unchanged(db, before[side], processes)
        assert exports["writes"] == writes_after_settlement
        # Changed provider fields still win; baseline preservation is not a
        # permanent preference for locally created planning.
        with server.lock:
            exports["issue"].update(title="Later Linear title", updatedAt="2026-10-08T12:00:01Z")
        for side, db in enumerate(databases):
            exchange(
                side,
                lambda: (
                    db.execute(
                        "SELECT issue_title FROM tasks WHERE id=?", (peer["task"],)
                    ).fetchone()
                    == ("Later Linear title",)
                ),
            )
            assert db.execute(
                "SELECT count(*) FROM task_changes WHERE task_id=? AND field='name' "
                "AND value_json=? AND conflict_json IS NOT NULL AND acknowledged=0",
                (peer["task"], json.dumps("Saved after task creation")),
            ).fetchone() == (1,)
            _assert_execution_unchanged(db, before[side], processes)
        with server.lock:
            assert exports["links"] == [json.loads(captured_project)["link_id"]]
            assert exports["writes"] == writes_after_settlement
            assert not server.state["unexpected"], server.state["unexpected"]
    except Exception as error:
        raise AssertionError(
            {
                "writes": exports.get("writes"),
                "receipts": [
                    [receipt(side, kind) for kind in ("project", "task")] for side in range(2)
                ],
                "changes": [
                    [
                        db.execute(
                            "SELECT field,value_json,base_json,attempted,"
                            "acknowledged,conflict_json "
                            f"FROM {kind}_changes WHERE {kind}_id=?",
                            (peer[kind],),
                        ).fetchall()
                        for kind in ("project", "task")
                    ]
                    for db in databases
                ],
                "unexpected": server.state["unexpected"],
            }
        ) from error
    finally:
        for db in databases:
            db.close()


def _exercise_effects(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    db = sqlite3.connect(Path(fixture["home"]) / "loopflow.db", timeout=5)

    processes = ("00000000-0000-4000-8000-000000000001",)
    before = _execution_rows(db, processes)
    checkout = db.execute("SELECT worktree FROM tasks WHERE id=?", (fixture["task"],)).fetchone()
    _run(fixture, env, "task", "delete", fixture["task"])
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


def _replay_planning_document(fixture: dict, env: dict, reference: str, earlier: str) -> str:
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
    return revision


def _exercise_associated_creations(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    sides = _peer_sides(fixture, env)
    peer = sides[1][0]
    databases = [sqlite3.connect(Path(f["home"]) / "loopflow.db", timeout=5) for f, _ in sides]
    owners = [(fixture["local_project"], fixture["task"]), (peer["project"], peer["task"])]
    processes = ("00000000-0000-4000-8000-000000000001",)
    before = [_execution_rows(db, processes) for db in databases]
    checkout_sql = "SELECT worktree,workspace_slug,branch,base_commit FROM tasks WHERE id=?"
    checkouts = [
        db.execute(checkout_sql, (owners[i][1],)).fetchone() for i, db in enumerate(databases)
    ]
    assert all(
        before[i][table]
        for i in range(2)
        for table in (
            "agent_sessions",
            "processes",
            "task_workflows",
            "task_workflow_moves",
            "task_prs",
            "work_placements",
        )
    )
    captured = {
        (kind, origin): json.loads(body)
        for db in databases
        for kind, origin, body in db.execute(
            "SELECT kind,origin_id,export_json FROM planning_creations"
        )
    }

    def run(side: int, *args: str) -> str:
        return _run(*sides[side], *args, timeout=45)

    def status(side: int) -> dict:
        return _planning_status(*sides[side])

    def exchange(side: int, predicate, *, wait_for_publication: bool = True) -> None:
        _exchange(*sides[side], predicate, wait_for_publication=wait_for_publication)

    def receipts(side: int) -> dict:
        return {
            (kind, origin): (owner, json.loads(body), attempted, linked, acknowledged)
            for kind, origin, owner, body, attempted, linked, acknowledged in databases[
                side
            ].execute(
                "SELECT kind,origin_id,COALESCE(task_id,project_id),export_json,"
                "export_attempted,export_link_attempted,export_acknowledged "
                "FROM planning_creations WHERE origin_id IN (SELECT value FROM json_each(?))",
                (json.dumps([origin for pair in owners for origin in pair]),),
            )
        }

    def assert_receipts(side: int, settled: bool) -> None:
        rows = receipts(side)
        expected = {
            key: body
            for key, body in captured.items()
            if not (fixture["private"] and side == 0 and key[1] in owners[1])
        }
        assert rows.keys() == expected.keys(), rows
        for (kind, origin), body in expected.items():
            index = 0 if kind == "project" else 1
            # Only the first origin has affirmative provider evidence. A mapping
            # cannot confirm the other origin's different attempted UUID.
            acknowledged = int(settled and origin == owners[0][index])
            assert rows[kind, origin] == (
                owners[side][index],
                body,
                1,
                int(kind == "project"),
                acknowledged,
            ), rows
        _assert_execution_unchanged(databases[side], before[side], processes)
        assert (
            databases[side].execute(checkout_sql, (owners[side][1],)).fetchone() == checkouts[side]
        )

    def saved_title(side: int) -> str:
        return (
            databases[side]
            .execute("SELECT issue_title FROM tasks WHERE id=?", (owners[side][1],))
            .fetchone()[0]
        )

    try:
        with server.lock:
            server.state.update(associated_creations=True, creation_writes=[], offline=True)
            server.state["issues"] = server.state["issues"][:1]
        subprocess.run(
            ["git", "remote", "add", "plans", fixture["remote"]], cwd=fixture["repo"], check=True
        )
        for side in range(2):
            destination = run(
                side, "planning", "connect", "--remote", "plans", "--shared", "origins"
            ).strip()
            if not (fixture["private"] and side == 1):
                waves = [fixture["wave"]]
                if fixture["private"]:
                    waves.append(fixture["independent_wave"])
                run(side, "planning", "select", destination, "--wave", *waves)
        if fixture["private"]:
            run(0, "wave", "ensure", "independent")
            independent = json.loads(
                run(
                    0,
                    "task",
                    "create",
                    "--wave",
                    "independent",
                    "--title",
                    "Independent before readback",
                    "--json",
                )
            )["id"]
            # Explicit local Project activation creates placement/transition rows.
            # Preserve every prior row, then freeze the exchange boundary after setup.
            prepared = _execution_rows(databases[0], processes)
            for table, rows in before[0].items():
                if table in ("work_placements", "project_transitions"):
                    assert all(row in prepared[table] for row in rows), table
                else:
                    assert prepared[table] == rows, table
            before[0] = prepared
        exchange(0, lambda: status(0)["publication_state"] == "confirmed")
        for side in [1] if fixture["private"] else [1, 0]:
            exchange(side, lambda side=side: bool(status(side)["conflicts"]))
            for index, provider in enumerate([fixture["project"], fixture["issue"]]):
                run(
                    side,
                    "planning",
                    "associate",
                    owners[1 - side][index],
                    "--with",
                    owners[side][index],
                    "--linear",
                    provider,
                )
            exchange(side, lambda side=side: len(receipts(side)) == 4)
            assert_receipts(side, False)
        if fixture["private"]:
            earlier = status(0)["publication_revision"]
            run(1, "task", "edit", peer["task"], "--title", "Private after association")
            private_changes = (
                databases[1]
                .execute(
                    "SELECT id,value_json,base_json,attempted,acknowledged FROM task_changes "
                    "WHERE task_id=? ORDER BY id",
                    (peer["task"],),
                )
                .fetchall()
            )
            assert private_changes
            run(0, "task", "edit", independent, "--title", "Independent after readback")

            def independent_title(side: int) -> str | None:
                row = (
                    databases[side]
                    .execute("SELECT issue_title FROM tasks WHERE id=?", (independent,))
                    .fetchone()
                )
                return row[0] if row else None

            def private_preserved() -> None:
                assert_receipts(0, True)
                assert_receipts(1, True)
                assert saved_title(1) == "Private after association"
                assert (
                    databases[1]
                    .execute(
                        "SELECT id,value_json,base_json,attempted,acknowledged FROM task_changes "
                        "WHERE task_id=? ORDER BY id",
                        (peer["task"],),
                    )
                    .fetchall()
                    == private_changes
                )
                reading = status(1)
                assert reading["conflicts"]
                for identity in [peer["wave"], *owners[1]]:
                    record = next(r for r in reading["records"] if r["object"]["id"] == identity)
                    assert record["destination"] is None, record
                assert independent_title(1) == "Independent after readback"
                document = subprocess.check_output(
                    [
                        "git",
                        "--git-dir",
                        fixture["remote"],
                        "show",
                        "refs/loopflow/planning/shared/origins:planning.json",
                    ],
                    text=True,
                )
                for value in [peer["wave"], *owners[1], "Private after association"]:
                    assert value not in document, value
                for side_fixture, _ in sides:
                    assert not (Path(side_fixture["home"]) / "provider-started").exists()

            with server.lock:
                server.state["offline"] = False
            exchange(0, lambda: all(row[-1] == 1 for row in receipts(0).values()))
            # The private projection still retains the selected origin's receipt.
            # Its acknowledgement may propagate; the private UUID cannot borrow it.
            exchange(
                1,
                lambda: (
                    independent_title(1) == "Independent after readback"
                    and all(
                        receipts(1)[kind, owners[0][index]][-1] == 1
                        for index, kind in enumerate(["project", "task"])
                    )
                ),
            )
            private_preserved()
            for _ in range(2):
                revision = _replay_planning_document(
                    fixture, env, "refs/loopflow/planning/shared/origins", earlier
                )
                lock = Path(peer["home"]) / "locks/planning-peers" / f"{status(1)['id']}.lock"
                with lock.open("a") as handle:
                    fcntl.flock(handle, fcntl.LOCK_EX)
                    exchange(
                        1,
                        lambda: status(1)["imported_revision"] == revision,
                        wait_for_publication=False,
                    )
                    private_preserved()
            with server.lock:
                assert not server.state["creation_writes"], server.state["creation_writes"]
                assert not server.state["unexpected"], server.state["unexpected"]
            return
        exchange(0, lambda: status(0)["pending_local"] is False)
        earlier = status(0)["publication_revision"]
        # A subsequent public save must survive unchanged readback on either
        # physical owner, including the one that did not capture the matched UUID.
        run(1, "task", "edit", peer["task"], "--title", "Saved after both attempts")
        exchange(0, lambda: saved_title(0) == "Saved after both attempts")
        with server.lock:
            server.state["offline"] = False
        for side in [1, 0, 1]:
            exchange(
                side,
                lambda side=side: all(
                    receipts(side)[kind, owners[0][index]][-1] == 1
                    for index, kind in enumerate(["project", "task"])
                ),
            )
            assert_receipts(side, True)
            assert saved_title(side) == "Saved after both attempts"
            assert databases[side].execute(
                "SELECT count(*) FROM task_changes WHERE task_id=? AND field='name' "
                "AND value_json=? AND acknowledged=0 AND conflict_json IS NULL",
                (owners[side][1], json.dumps("Saved after both attempts")),
            ).fetchone() == (1,)
            for task in [fixture["task"], peer["task"]]:
                assert (
                    json.loads(run(side, "task", "status", task, "--json"))["execution"]["task_id"]
                    == owners[side][1]
                )
        # Reversed/repeated older receipts cannot revoke exact acknowledgement
        # or acknowledge the unmatched origin. Acquisition bypasses publication.
        for side in [1, 0, 1]:
            revision = _replay_planning_document(
                fixture, env, "refs/loopflow/planning/shared/origins", earlier
            )
            lock = (
                Path(sides[side][0]["home"]) / "locks/planning-peers" / f"{status(side)['id']}.lock"
            )
            with lock.open("a") as handle:
                fcntl.flock(handle, fcntl.LOCK_EX)
                exchange(
                    side,
                    lambda side=side: status(side)["imported_revision"] == revision,
                    wait_for_publication=False,
                )
                assert_receipts(side, True)
                assert saved_title(side) == "Saved after both attempts"
        with server.lock:
            assert not server.state["creation_writes"], server.state["creation_writes"]
            assert not server.state["unexpected"], server.state["unexpected"]
    finally:
        for db in databases:
            db.close()


def _exercise_associations(fixture: dict, env: dict, server: ThreadingHTTPServer) -> None:
    repo = Path(fixture["repo"])
    sides = _peer_sides(fixture, env)
    peer = sides[1][0]
    databases = [sqlite3.connect(Path(f["home"]) / "loopflow.db", timeout=5) for f, _ in sides]
    processes = ("00000000-0000-4000-8000-000000000001",)

    def run(side: int, *args: str) -> str:
        return _run(*sides[side], *args, timeout=45)

    def exchange(side: int, predicate) -> None:
        _exchange(*sides[side], predicate)

    def status(side: int) -> dict:
        return _planning_status(*sides[side])

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
    for side in [1, 0, 1]:
        revision = _replay_planning_document(fixture, env, reference, earlier)
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
        marker = ': > "$HOME/provider-started"\n' if name != "gh" else ""
        stub.write_text(f"#!/bin/sh\n{marker}exit 1\n")
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
            elif sys.argv[2] == "associated-creations":
                _exercise_associated_creations(fixture, env, server)
            elif sys.argv[2] == "creation-origins":
                _exercise_creation_origins(fixture, env, server)
            elif sys.argv[2] == "ordering":
                _exercise_ordering(fixture, env, server)
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
