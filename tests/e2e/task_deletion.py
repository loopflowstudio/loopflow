"""Run the real CLI through a local HTTPS proxy with synthetic Linear state."""

import json
import os
import sqlite3
import ssl
import subprocess
import sys
import threading
import time
import uuid
from datetime import datetime, timedelta
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def _next_revision(value: str) -> str:
    return (datetime.fromisoformat(value) + timedelta(seconds=1)).isoformat()


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
        query, variables = request["query"], request.get("variables", {})
        state = self.server.state
        project = state["project"]
        issue = state["issue"]
        page = {"hasNextPage": False, "endCursor": None}
        if "query ListTeams" in query:
            data = {
                "teams": {
                    "nodes": [
                        {
                            "id": "team-task-pr-tests",
                            "name": "Fixture",
                            "key": "INF",
                            "description": state["claim"],
                        }
                    ]
                }
            }
        elif "query IssueOwnership" in query:
            data = {"issue": issue if not issue["trashed"] else None}
        elif "query IssueAttachments" in query:
            data = {"issue": {"attachments": {"nodes": [], "pageInfo": page}}}
        elif "query ListInitiatives" in query:
            data = {
                "initiatives": {
                    "nodes": [
                        {
                            "id": "initiative-task-pr-tests",
                            "name": "Task PR Tests",
                            "description": "",
                        }
                    ],
                    "pageInfo": page,
                }
            }
        elif "mutation UpdateInitiative" in query:
            data = {"initiativeUpdate": {"initiative": {"id": variables["id"]}}}
        elif "mutation RenameProject" in query:
            project["name"] = variables["name"]
            project["updatedAt"] = _next_revision(project["updatedAt"])
            data = {"projectUpdate": {"success": True}}
        elif "mutation UpdateProject" in query:
            project.update({key: variables[key] for key in ["name", "description", "content"]})
            project["updatedAt"] = _next_revision(project["updatedAt"])
            data = {"projectUpdate": {"project": {"id": variables["id"]}}}
        elif "query ListInitiativeProjects" in query:
            data = {"initiative": {"projects": {"nodes": [project], "pageInfo": page}}}
        elif "query ListProjectIssues" in query:
            data = {
                "project": {
                    "issues": {"nodes": [] if issue["trashed"] else [issue], "pageInfo": page}
                }
            }
        else:
            raise AssertionError(query)
        body = json.dumps({"data": data}).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def _abandon_retains_execution(
    fixture: dict, repo: Path, env: dict, db: sqlite3.Connection, issue: dict, authored: Path
) -> None:
    process = str(uuid.uuid4())
    db.execute(
        "INSERT INTO processes(lfid,trace_id,cwd,started_at) VALUES(?,?,?,?)",
        (process, process, str(repo), int(time.time())),
    )
    db.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,"
        "interactive,task_id,wave_id,cwd) "
        "VALUES('retained-session','retained','generated',1,0,0,?,?,?)",
        (fixture["task"], fixture["wave"], str(repo)),
    )
    capture = db.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) "
        "VALUES('retained-session','captured',?,1,'{}')",
        (uuid.uuid4().hex,),
    ).lastrowid
    db.execute(
        "UPDATE agent_sessions SET current_capture=? WHERE id='retained-session'", (capture,)
    )
    db.commit()
    processes = db.execute("SELECT * FROM processes WHERE lfid=?", (process,)).fetchall()
    sessions = db.execute("SELECT * FROM agent_sessions WHERE id='retained-session'").fetchall()
    prs = db.execute("SELECT * FROM task_prs").fetchall()
    delivery = None
    for _ in range(2):
        result = subprocess.run(
            [fixture["lf"], "task", "abandon", "INF-123"],
            cwd=repo,
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode == 0, result.stderr
        assert issue["state"]["type"] == "unstarted"
        assert db.execute("SELECT abandoned_at IS NOT NULL FROM tasks").fetchone() == (1,)
        pending = db.execute(
            "SELECT id,target,attempted,settled FROM task_state_deliveries WHERE task_id=?",
            (fixture["task"],),
        ).fetchall()
        assert len(pending) == 1 and pending[0][1:] == ("canceled", 0, 0)
        if delivery is not None:
            assert pending == delivery
        delivery = pending
        assert (
            db.execute("SELECT * FROM processes WHERE lfid=?", (process,)).fetchall() == processes
        )
        assert (
            db.execute("SELECT * FROM agent_sessions WHERE id='retained-session'").fetchall()
            == sessions
        )
        assert db.execute("SELECT * FROM task_prs").fetchall() == prs
        assert authored.read_text() == "preserve authored work\n"


def main() -> None:
    fixture = json.loads(Path(sys.argv[1]).read_text())
    root, repo = Path(fixture["home"]), Path(fixture["repo"])
    cert, key = root / "cert.pem", root / "tls.key"
    subprocess.run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-keyout",
            str(key),
            "-out",
            str(cert),
            "-subj",
            "/CN=api.linear.app",
            "-addext",
            "subjectAltName=DNS:api.linear.app",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ],
        check=True,
        capture_output=True,
    )
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    leaf, leaf_key, csr = root / "leaf.pem", root / "leaf.key", root / "leaf.csr"
    subprocess.run(
        [
            "openssl",
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(leaf_key),
            "-out",
            str(csr),
            "-subj",
            "/CN=api.linear.app",
        ],
        check=True,
        capture_output=True,
    )
    extensions = root / "extensions"
    extensions.write_text("basicConstraints=critical,CA:FALSE\nsubjectAltName=DNS:api.linear.app\n")
    subprocess.run(
        [
            "openssl",
            "x509",
            "-req",
            "-in",
            str(csr),
            "-CA",
            str(cert),
            "-CAkey",
            str(key),
            "-CAcreateserial",
            "-out",
            str(leaf),
            "-days",
            "1",
            "-extfile",
            str(extensions),
        ],
        check=True,
        capture_output=True,
    )
    tls.load_cert_chain(leaf, leaf_key)
    subprocess.run(
        ["git", "remote", "set-url", "origin", "https://github.com/loopflowstudio/fixture.git"],
        cwd=repo,
        check=True,
    )
    (repo / ".lf").mkdir(exist_ok=True)
    (repo / ".lf/config.yaml").write_text(
        "pm:\n  provider: linear\n  linear_team: team-task-pr-tests\n"
    )
    wave = repo / "wave/task-pr-tests"
    wave.mkdir(parents=True, exist_ok=True)
    (wave / "GOAL.md").write_text(
        "---\npm:\n  linear_initiative: initiative-task-pr-tests\n---\nKeep work.\n"
    )
    authored = repo / "authored.txt"
    authored.write_text("preserve authored work\n")
    project = {
        "id": fixture["project"],
        "updatedAt": "2026-09-30T00:00:00Z",
        "name": "Task PR Tests",
        "description": "",
        "content": "workflow: feature",
        "status": {"type": "started"},
        "initiatives": {"nodes": [{"id": "initiative-task-pr-tests"}]},
        "teams": {"nodes": [{"id": "team-task-pr-tests"}]},
    }
    issue = {
        "id": fixture["issue"],
        "updatedAt": "2026-09-30T00:00:00Z",
        "identifier": "INF-123",
        "title": "Keep history",
        "description": "",
        "url": None,
        "sortOrder": 1,
        "completedAt": None,
        "prioritySortOrder": 1,
        "assignee": None,
        "state": {"type": "unstarted"},
        "team": {"id": "team-task-pr-tests"},
        "project": project,
        "trashed": False,
    }
    dbpath = root / "loopflow.db"
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith("LF_")
        and k.lower() not in ("http_proxy", "https_proxy", "all_proxy", "no_proxy")
    }
    env.update(
        LF_HOME=str(root),
        LF_PROVIDER_TOKEN_KEY_PATH=str(root / "provider.key"),
        SSL_CERT_FILE=str(cert),
        SSL_CERT_DIR=str(root / "empty-certs"),
    )
    (root / "empty-certs").mkdir()
    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server, sqlite3.connect(dbpath) as db:
        server.tls = tls
        server.state = {
            "project": project,
            "issue": issue,
            "claim": "<!-- loopflow-repository: loopflowstudio/fixture -->",
            "deletes": 0,
        }
        env["HTTPS_PROXY"] = f"http://127.0.0.1:{server.server_port}"
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            _abandon_retains_execution(fixture, repo, env, db, issue, authored)
        finally:
            server.shutdown()
            thread.join()
    print("real CLI: cancellation saved locally with execution and files preserved")


if __name__ == "__main__":
    main()
