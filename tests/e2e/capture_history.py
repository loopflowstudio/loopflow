"""Prove capture preservation with published v0.13.3 and a source candidate.

Only ordinary commands run, in a temporary LF_HOME with stub providers. No
installation, account import, production Machine access or layout conversion.
"""

import argparse
import hashlib
import json
import os
import platform
import sqlite3
import subprocess
import tarfile
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKSUMS = {
    ("Darwin", "arm64"): "b400c12b7f0f793e9d9f31602999abb27fdd971796deec1bfeb8d9b00c1b7639",
    ("Darwin", "x86_64"): "c3a5d5f7293745884fdf83636e694707fd799e19bb9fbc93dfd52976a3f0c512",
    ("Linux", "aarch64"): "086f0903d2c7e0357f4bad69e55773f522ca8b641ee339ac0768ebb75483e7ff",
    ("Linux", "x86_64"): "5c567831c734341e0e3cb4c99ae057f217555ad24dc6fa40925af4fc926dcd43",
}


def _command(binary: Path, env: dict[str, str], cwd: Path, *args: str) -> str:
    result = subprocess.run(
        [str(binary), *args],
        env={**env, "LF_BIN": str(binary)},
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert result.returncode == 0, f"{args}: {result.stderr}\n{result.stdout}"
    return result.stdout


def _captures(home: Path) -> dict[str, bytes]:
    return {
        str(path.relative_to(home)): path.read_bytes()
        for path in (home / "runs").glob("*/*/*")
        if path.is_file()
        and path.name in {"manifest.json", "context.json", "events.jsonl", "terminal.json"}
    }


def _prove(archive: Path, candidate: Path, root: Path) -> None:
    expected = CHECKSUMS[(platform.system(), platform.machine())]
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == expected
    prior_dir = root / "prior"
    prior_dir.mkdir()
    with tarfile.open(archive) as package:
        package.extract("lf", prior_dir, filter="data")
    prior = prior_dir / "lf"
    home, repo, bin_dir = root / "home", root / "repo", root / "home/bin"
    bin_dir.mkdir(parents=True)
    repo.mkdir()
    env = {
        name: value
        for name, value in os.environ.items()
        if not name.startswith(("LF_", "LOOPFLOW_", "CLAUDE", "CODEX", "OPENAI", "ANTHROPIC"))
    }
    env.update(
        HOME=str(home),
        LF_HOME=str(home),
        XDG_CONFIG_HOME=str(home / "config"),
        XDG_DATA_HOME=str(home / "data"),
        PATH=f"{bin_dir}:{env.get('PATH', '')}",
        RUST_LOG="off",
    )
    for args in [
        ("init", "-q"),
        (
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@localhost",
            "commit",
            "--allow-empty",
            "-qm",
            "Fixture",
        ),
    ]:
        subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True)
    # The released fixture provider reads its released variable; the candidate
    # provider reads only the new capture context. No compatibility reader ships.
    provider_source = (ROOT / "rust/loopflow/tests/support/opencode_server.py").read_text()
    provider = bin_dir / "opencode"
    provider.write_text(
        provider_source.replace("__WAIT__", "False").replace("LF_CAPTURE_KEY", "LF_RUN_ID")
    )
    provider.chmod(0o755)
    tmux = bin_dir / "tmux"
    tmux.write_text('#!/bin/sh\nif [ "$1" = has-session ]; then exit 1; fi\nexit 0\n')
    tmux.chmod(0o755)
    _command(
        prior, env, repo, "--model", "opencode", ":", "Retain this history", "-b", "--no-loopflow"
    )
    database = home / "loopflow.db"
    with sqlite3.connect(database) as db:
        session, capture, native = db.execute(
            "SELECT s.id,e.receipt_key,s.provider_thread FROM agent_sessions s "
            "JOIN session_events e ON e.seq=s.current_capture"
        ).fetchone()
    assert native, "released CLI must record a native conversation"
    retained = _captures(home)
    assert any(name.endswith("manifest.json") for name in retained)
    assert any(name.endswith("events.jsonl") for name in retained)
    prior_usage = json.loads(_command(prior, env, repo, "usage", "--json"))
    definitions = repo / ".lf"
    (definitions / "skills").mkdir(parents=True)
    (definitions / "flows").mkdir()
    (definitions / "skills/review-proof.md").write_text("Review retained history.")
    (definitions / "flows/review-first.yaml").write_text(
        "- step:\n    id: review\n    name: review-proof\n    human: true\n"
    )
    waiting = subprocess.run(
        [str(prior), "--model", "opencode", "flow", "review-first", "-b", "--no-loopflow"],
        env={**env, "LF_BIN": str(prior)},
        cwd=repo,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert "waiting for human input" in waiting.stderr, waiting.stderr
    with sqlite3.connect(database) as db:
        review, flow, review_capture = db.execute(
            "SELECT s.id,s.flow_session_id,e.receipt_key FROM agent_sessions s "
            "JOIN session_events e ON e.seq=s.current_capture WHERE s.kind='flow_review'"
        ).fetchone()
    review_env = {
        **env,
        "LF_RUN_ID": review_capture,
        "LF_HUMAN_SESSION": json.dumps({"kind": "standalone_flow", "id": review}),
    }
    _command(prior, review_env, repo, "session", "ready", "Keep this released feedback")
    with sqlite3.connect(database) as db:
        history = db.execute("SELECT * FROM session_events ORDER BY seq").fetchall()
    before_reads = _captures(home)
    provider.write_text(provider_source.replace("__WAIT__", "False"))
    # Pin nested commands as well as the explicit public operations.
    (bin_dir / "lf").symlink_to(candidate)
    listed = json.loads(
        _command(
            candidate,
            env,
            repo,
            "session",
            "list",
            "--all",
            "--history",
            "--interactive",
            "all",
            "--json",
        )
    )
    assert session in {row["id"] for row in listed}
    _command(candidate, env, repo, "session", "history", session, "--json")
    _command(candidate, env, repo, "mon", "show", session, "--input", capture)
    usage = json.loads(_command(candidate, env, repo, "usage", "--json"))
    retained_usage = next(row for row in prior_usage if row["session_id"] == session)
    assert next(row for row in usage if row["session_id"] == session) == retained_usage
    with sqlite3.connect(database) as db:
        assert db.execute("SELECT * FROM session_events ORDER BY seq").fetchall() == history
    assert _captures(home) == before_reads, "public reads changed capture bytes or paths"
    _command(candidate, env, repo, "replay", capture)
    _command(candidate, env, repo, "session", "connect", session)
    with sqlite3.connect(database) as db:
        assert (
            db.execute(
                "SELECT provider_thread FROM agent_sessions WHERE id=?", (session,)
            ).fetchone()[0]
            == native
        )
        assert (
            db.execute(
                "SELECT * FROM session_events ORDER BY seq LIMIT ?", (len(history),)
            ).fetchall()
            == history
        )
        assert (
            db.execute("SELECT ready_summary FROM agent_sessions WHERE id=?", (review,)).fetchone()[
                0
            ]
            == "Keep this released feedback"
        )
    provider.write_text(provider_source.replace("__WAIT__", "True"))
    launches_before = (home / "launched").read_text().splitlines()
    with (root / "review.log").open("w+") as log:
        opened = subprocess.Popen(
            [str(candidate), "session", "connect", review],
            cwd=repo,
            env={**env, "LF_BIN": str(candidate)},
            stdin=subprocess.PIPE,
            stdout=log,
            stderr=log,
            text=True,
        )
        try:
            deadline = time.monotonic() + 60
            while (home / "launched").read_text().splitlines() == launches_before:
                if opened.poll() is not None or time.monotonic() >= deadline:
                    log.seek(0)
                    raise AssertionError(log.read())
                time.sleep(0.02)
            # Completion waits for launch publication under the Session lock,
            # then stops its client. A provider's launch log precedes readiness.
            _command(candidate, env, repo, "session", "complete", review)
            opened.communicate(timeout=60)
            log.seek(0)
            assert opened.returncode == 0, log.read()
        finally:
            if opened.poll() is None:
                opened.kill()
                opened.wait()
    provider.write_text(provider_source.replace("__WAIT__", "False"))
    _command(candidate, env, repo, "flow", "resume", flow)
    with sqlite3.connect(database) as db:
        assert (
            db.execute("SELECT state FROM flow_sessions WHERE id=?", (flow,)).fetchone()[0]
            == "completed"
        )
        feedback, completed = db.execute(
            "SELECT ready_summary,completed_at FROM agent_sessions WHERE id=?", (review,)
        ).fetchone()
        assert feedback == "Keep this released feedback"
        assert completed is not None
    # A provider-issued nested direct invocation gets its own Session and the
    # parent's exact Process, without conflating either with the capture key.
    (home / "tool-command.json").write_text(
        json.dumps(
            {
                "argv": [
                    str(candidate),
                    "--tui",
                    "--agent",
                    "opencode",
                    ":",
                    "Nested work",
                ],
                "cwd": str(repo),
            }
        )
    )
    _command(candidate, env, repo, "--tui", "--agent", "opencode", ":", "Parent work")
    assert json.loads((home / "tool-result.json").read_text())["code"] == 0
    with sqlite3.connect(database) as db:
        parents = db.execute(
            "SELECT e.parent_lf_process_id,s.provider_lf_process_id FROM processes e "
            "JOIN agent_sessions s ON s.id=e.caller_session_id"
        ).fetchall()
        assert parents and all(parent == owner for parent, owner in parents)
    for name, data in retained.items():
        # Native resume may append events to its original capture; its immutable
        # request and all earlier event bytes must still be present at that path.
        current = (home / name).read_bytes()
        assert current.startswith(data) if name.endswith("events.jsonl") else current == data, name
    assert not (home / "captures").exists()
    print(
        "PASS: released history, paths, native resume, replay, review feedback/settlement "
        "and nested Process ancestry preserved"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--released-archive", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="lf-capture-history-") as temporary:
        _prove(args.released_archive.resolve(), args.candidate.resolve(), Path(temporary))


if __name__ == "__main__":
    main()
