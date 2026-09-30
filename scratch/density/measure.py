"""Measure a synthetic current-schema store; never open an installed Home."""

import hashlib
import json
import math
import os
import platform
import sqlite3
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target/debug/lf"
PROBE = ROOT / "target/debug/examples/density_probe"
OUTPUT = ROOT / ".lf/tmp/density-final"
ENV = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "LOOPFLOW_"))}


def _run(binary: Path, args: list[str], home: Path) -> tuple[bytes, bytes, float]:
    env = {
        **ENV,
        "LF_HOME": str(home),
        "LF_DB_PATH": str(home / "db"),
        "LF_BIN": str(BINARY),
        "RUST_LOG": "off",
    }
    start = time.perf_counter()
    result = subprocess.run(
        [str(binary), *args], cwd=home, env=env, capture_output=True, timeout=120
    )
    elapsed = (time.perf_counter() - start) * 1000
    if result.returncode:
        raise RuntimeError(f"{args}: {result.stderr.decode()}")
    return result.stdout, result.stderr, elapsed


def _stats(samples: list[float]) -> dict:
    warm = sorted(samples[1:])
    return {
        "first_ms": samples[0],
        "warm_p50_ms": statistics.median(warm),
        "warm_p95_ms": warm[math.ceil(len(warm) * 0.95) - 1],
        "samples_ms": samples,
    }


def _seed(home: Path) -> dict:
    invocation = json.loads(_run(PROBE, ["template"], home)[0])
    # A saved skill body belongs to the graph payload, not the inventory page.
    invocation["steps"][0]["Skill"]["skill"]["content"] = "Inspect the retained evidence.\n" * 160
    with sqlite3.connect(home / "db") as db:
        db.execute("PRAGMA foreign_keys=ON")
        db.executescript("""
            CREATE TEMP TABLE numbers AS WITH RECURSIVE n(i) AS
            (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<100000) SELECT i FROM n;
            INSERT INTO execs(id,trace_id,parent_exec_id,via_agent,command,repo,cwd,
                started_at,outcome,completed_at,exit_code)
            SELECT printf('00000000-0000-4000-8000-%012x',i),
                printf('10000000-0000-4000-8000-%012x',i),
                CASE WHEN i>1 THEN '00000000-0000-4000-8000-000000000001' END,0,
                '["lf","pr","checks"]','/fixture/repo-'||(i%3),'/fixture',i/10,
                CASE WHEN i%4=0 THEN NULL WHEN i%4=1 THEN 'failed' ELSE 'succeeded' END,
                CASE WHEN i%4!=0 THEN i/10+1 END,
                CASE WHEN i%4=1 THEN 42 WHEN i%4!=0 THEN 0 END FROM numbers;
            INSERT INTO waves(id,name,repo,created_at)
            VALUES('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa','fixture','/fixture/repo-0',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at)
            VALUES('proj_00000000000000000000000000000001',
                'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa','fixture-project',1);
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,worktree)
            SELECT printf('task_%032x',i),'proj_00000000000000000000000000000001',
                'issue-'||i,'DENSE-'||i,1,'/fixture/task-'||i
            FROM numbers WHERE i<=1000;
            INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,
                cwd,task_id,wave_id,
                interactive,kind,completed_at,repo,provider,model)
            SELECT 'session-'||i,'Fixture '||i,'human',i,1,'/fixture',printf('task_%032x',1+i%1000),
                'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',i%2,
                CASE WHEN i%4=0 THEN 'flow_review' WHEN i%4=1 THEN 'ask' ELSE 'conversation' END,
                CASE WHEN i%3=0 THEN i+1 END,'/fixture/repo-0','codex','fixture'
                FROM numbers WHERE i<=20000;
            INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
            SELECT 'session-'||i,'captured',printf('20000000-0000-4000-8000-%012x',i),1,
                json_object('context','synthetic '||hex(zeroblob(2048)))
            FROM numbers WHERE i<=20000;
            UPDATE agent_sessions SET current_capture=(SELECT seq FROM session_events
                WHERE session_id=agent_sessions.id);
            INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,
                receipt_key,exec_id,
                task_id,wave_id,observed_at,payload,captured_event)
            SELECT 'session-'||(1+(i-1)%20000),'thread','turn-'||i,'output','event-'||i,
                printf('00000000-0000-4000-8000-%012x',i),printf('task_%032x',1+(1+(i-1)%20000)%1000),
                'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',i,json_object('text',hex(zeroblob(512))),
                (SELECT current_capture FROM agent_sessions
                 WHERE id='session-'||(1+(i-1)%20000)) FROM numbers;
        """)
        db.executemany(
            """
            INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,
                position_version,worker_generation,updated_at,state,ended_at,pending_session_id)
            VALUES(?,?,?,?,0,0,1,0,?,?,?,?)
        """,
            [
                (
                    f"flow-{i}",
                    f"task_{1 + i * 4 % 1000:032x}",
                    "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                    json.dumps({**invocation, "id": f"flow-{i}"}),
                    i,
                    "completed" if i % 2 else "current",
                    i + 1 if i % 2 else None,
                    f"session-{i * 4}" if i % 2 == 0 else None,
                )
                for i in range(1, 5001)
            ],
        )
        db.execute("""UPDATE agent_sessions SET flow_session_id=(SELECT id FROM flow_sessions
            WHERE pending_session_id=agent_sessions.id) WHERE kind='flow_review'""")
        db.execute("""
            INSERT INTO flow_events(flow_id,node,iterations,kind,exec_id,payload)
            SELECT 'flow-'||i,0,'[]','operation_started',
                printf('00000000-0000-4000-8000-%012x',i),'{}'
            FROM numbers WHERE i<=5000""")
        assert not db.execute("PRAGMA foreign_key_check").fetchall()
        counts = {
            name: db.execute(f"SELECT count(*) FROM {name}").fetchone()[0]
            for name in (
                "waves",
                "projects",
                "tasks",
                "execs",
                "agent_sessions",
                "flow_sessions",
                "session_events",
                "flow_events",
            )
        }
    return counts


def main() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    report = {
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "binary_sha256": hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        "platform": platform.platform(),
        "load": os.getloadavg(),
        "commands": {},
    }
    with tempfile.TemporaryDirectory(prefix="loo298-density-") as directory:
        home = Path(directory).resolve()
        _run(BINARY, ["session", "list", "--all", "--json"], home)
        startup = ["--version"]
        empty = ["session", "list", "--all", "--json", "--limit", "100"]
        for name, args in (("startup_version", startup), ("startup_empty_session_list", empty)):
            report[name] = _stats([_run(BINARY, args, home)[2] for _ in range(21)])
        report["counts"] = _seed(home)
        report["database_bytes"] = (home / "db").stat().st_size
        queries = {
            "session_list": [
                "session",
                "list",
                "--all",
                "--page",
                "--json",
                "--limit",
                "100",
                "--interactive",
                "all",
            ],
            "session_detail": ["session", "history", "session-2", "--json", "--limit", "100"],
            "flow_list": ["flow", "list", "--sessions", "--all", "--json", "--limit", "100"],
            "flow_detail": ["flow", "show", "flow-2", "--sessions", "--json"],
            "exec_list": ["exec", "list", "--all", "--json", "--limit", "100"],
            "exec_children_list": [
                "exec",
                "list",
                "--all",
                "--json",
                "--limit",
                "100",
                "--parent",
                "00000000-0000-4000-8000-000000000001",
            ],
            "exec_detail": ["exec", "show", "00000000-0000-4000-8000-000000000002", "--json"],
            "session_search_miss": [
                "session",
                "list",
                "--all",
                "--json",
                "--search",
                "NO_MATCH",
                "--limit",
                "100",
            ],
            "exec_search_miss": [
                "exec",
                "list",
                "--all",
                "--json",
                "--search",
                "NO_MATCH",
                "--limit",
                "100",
                "--parent",
                "00000000-0000-4000-8000-000000000001",
            ],
        }
        for name, args in queries.items():
            samples = []
            for _ in range(21):
                payload, _, elapsed = _run(BINARY, args, home)
                json.loads(payload)
                samples.append(elapsed)
            path = OUTPUT / f"{name}.json"
            path.write_bytes(payload)
            value = json.loads(payload)
            if name.endswith("_list"):
                assert len(value["entries"]) == 100 and value["next"], name
            elif name.endswith("search_miss"):
                assert not (value["entries"] if isinstance(value, dict) else value), name
            elif name == "session_detail":
                assert len(value) == 6 and all(row["session_id"] == "session-2" for row in value)
            elif name == "exec_detail":
                assert value["outcome"] == "succeeded" and value["exit_code"] == 0
            else:
                assert value["graph"]["name"] == "density"
            traced, trace, _ = _run(PROBE, args, home)
            (OUTPUT / f"{name}-traced.json").write_bytes(traced)
            traced_value = json.loads(traced)
            if name == "exec_list":
                # The real CLI records its own Exec and completes it after the
                # read. Only that result can differ in the later SQL probe.
                for original, later in zip(value["entries"], traced_value["entries"], strict=True):
                    for field in ("outcome", "completed_at", "exit_code"):
                        if original[field] != later[field]:
                            assert original[field] is None
                            original[field] = later[field]
            assert traced_value == value, name
            statements = json.loads(trace)
            # Replay actual expanded SELECT/WITH statements, including startup
            # reads. PRAGMA and writes are deliberately excluded from replay.
            reads = [s for s in statements if s.lstrip().upper().startswith(("SELECT", "WITH"))]
            sql_path = OUTPUT / f"{name}-sql.json"
            sql_path.write_text(json.dumps(reads, indent=2) + "\n")
            raw = json.loads(_run(PROBE, ["sql", str(home / "db"), str(sql_path)], home)[0])
            encode = json.loads(_run(PROBE, ["payload", str(path)], home)[0])
            report["commands"][name] = {
                "args": args,
                "cli": _stats(samples),
                "stdout_bytes": len(payload),
                "sql": {**_stats(raw["samples_ms"]), "rows": raw["rows"], "sqlite": raw["sqlite"]},
                "json_encode": _stats(encode),
                "select_count": len(reads),
            }
            (OUTPUT / "results.json").write_text(json.dumps(report, indent=2) + "\n")
            print(name, round(statistics.median(samples[1:]), 2), flush=True)
        with sqlite3.connect(home / "db") as db:
            report["final_exec_count"] = db.execute("SELECT count(*) FROM execs").fetchone()[0]
        report["final_database_bytes"] = (home / "db").stat().st_size
        (OUTPUT / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Results: {OUTPUT / 'results.json'}")


if __name__ == "__main__":
    main()
