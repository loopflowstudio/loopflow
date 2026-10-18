"""Exercise the candidate CLI against native OpenCode and a deterministic loopback model."""

import fcntl
import json
import os
import pty
import select
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import time
from collections.abc import Callable
from pathlib import Path

import pyte
from native_turns import Model
from probe_support import isolated_environment


def _routing_and_failure(
    run: Callable[[list[str]], subprocess.CompletedProcess[bytes]],
    model: Model,
    repo: Path,
    environment: dict[str, str],
) -> None:
    skills = repo / ".lf/skills"
    skills.mkdir()
    (skills / "route-proof.md").write_text("Select the alpha path in the declared JSON format.")
    (skills / "alpha-proof.md").write_text("SELECTED_ALPHA_PROOF")
    (skills / "zeta-proof.md").write_text("UNSELECTED_ZETA_PROOF")
    flows = repo / ".lf/flows"
    flows.mkdir()
    (flows / "routing-proof.yaml").write_text(
        "- xor:\n    router: route-proof\n    paths:\n"
        "      alpha:\n        description: Select this branch\n        skill: alpha-proof\n"
        "      zeta:\n        description: Leave this branch untouched\n        skill: zeta-proof\n"
    )
    model.answer = '{"path":"alpha"}'
    assert run(["-b", "flow", "routing-proof"]).returncode == 0
    requests = json.dumps(model.requests)
    assert "SELECTED_ALPHA_PROOF" in requests
    assert "UNSELECTED_ZETA_PROOF" not in requests

    model.answer = None
    # Native title generation can precede the actual turn; fail both requests.
    model.fail_all = True
    failed = run(["-b", "--wave", "fixture", ":", "Fail this explicit input."])
    assert failed.returncode != 0, "native model failure was reported as success"
    with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
        session, native = db.execute(
            "SELECT id,provider_thread FROM agent_sessions "
            "ORDER BY created_at DESC,rowid DESC LIMIT 1"
        ).fetchone()
        completions = db.execute(
            "SELECT payload FROM session_events WHERE session_id=? AND kind='completed'",
            (session,),
        ).fetchall()
    assert completions and all(json.loads(row[0])["status"] == "failed" for row in completions)
    model.fail_all = False
    assert (
        run(["-b", "session", "connect", session, "--message", "New explicit input."]).returncode
        == 0
    )
    with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
        assert (
            db.execute(
                "SELECT provider_thread FROM agent_sessions WHERE id=?", (session,)
            ).fetchone()[0]
            == native
        )
        statuses = [
            json.loads(row[0])["status"]
            for row in db.execute(
                "SELECT payload FROM session_events "
                "WHERE session_id=? AND kind='completed' ORDER BY seq",
                (session,),
            )
        ]
    assert statuses == ["failed", "completed"], statuses
    print(
        json.dumps(
            {
                "native_json_routing": True,
                "model_failure_rejected": True,
                "explicit_input_after_failure": True,
                "model_requests": len(model.requests),
            },
            indent=2,
        )
    )


def _provider_death(binary: str, repo: Path, environment: dict[str, str], model: Model) -> None:
    ledger = Path(environment["LF_HOME"]) / "loopflow.db"
    with sqlite3.connect(ledger) as db:
        session, native = db.execute(
            "SELECT id,provider_thread FROM agent_sessions "
            "ORDER BY created_at DESC,rowid DESC LIMIT 1"
        ).fetchone()
    model.hold = len(model.requests) + 1
    child = subprocess.Popen(
        [binary, "-b", "session", "connect", session, "--message", "Interrupt this input."],
        cwd=repo,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        assert model.arrived.wait(20), "resumed provider did not reach the model"
        with sqlite3.connect(ledger) as db:
            capture = db.execute(
                "SELECT current_capture FROM agent_sessions WHERE id=?", (session,)
            ).fetchone()[0]
            pid = db.execute(
                "SELECT pid FROM processes WHERE kind='agent' AND agent_session_id=? "
                "AND completed_at IS NULL ORDER BY started_at DESC LIMIT 1",
                (session,),
            ).fetchone()[0]
        os.kill(pid, signal.SIGKILL)
        _, error = child.communicate(timeout=20)
        assert child.returncode != 0, error.decode(errors="replace")
    finally:
        model.release.set()
        if child.poll() is None:
            child.terminate()
            child.communicate(timeout=10)
    model.hold = 0
    before = len(model.requests)
    resumed = subprocess.run(
        [binary, "-b", "session", "connect", session, "--message", "Explicit input after death."],
        cwd=repo,
        env=environment,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        timeout=90,
    )
    assert resumed.returncode == 0, resumed.stderr.decode(errors="replace")
    assert len(model.requests) > before
    with sqlite3.connect(ledger) as db:
        current_native, current_capture = db.execute(
            "SELECT provider_thread,current_capture FROM agent_sessions WHERE id=?", (session,)
        ).fetchone()
        assert current_native == native and current_capture != capture
        # Native receipts join through the frozen initiating-input origin.
        completions = (
            "SELECT done.payload FROM session_events done JOIN session_events origin "
            "ON origin.session_id=done.session_id AND origin.provider_thread=done.provider_thread "
            "AND origin.provider_turn=done.provider_turn AND origin.kind='started' "
            "WHERE origin.captured_event=? AND done.kind='completed'"
        )
        interrupted = db.execute(completions, (capture,)).fetchall()
        assert not any(json.loads(row[0])["status"] == "completed" for row in interrupted)
        completed = db.execute(completions, (current_capture,)).fetchall()
        assert completed and all(
            json.loads(row[0])["status"] == "completed" for row in completed
        ), completed
    print(
        json.dumps(
            {
                "explicit_input_after_provider_death": True,
                "native_identity_preserved": True,
                "interrupted_capture_not_completed": True,
            },
            indent=2,
        )
    )


def _terminal(
    binary: str,
    args: list[str],
    repo: Path,
    environment: dict[str, str],
    submit: bool,
    *,
    resize: bool = False,
) -> bytes:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 35, 120, 0, 0))
    process = subprocess.Popen(
        [binary, *args],
        cwd=repo,
        env={**environment, "TERM": "xterm-256color"},
        stdin=slave,
        stdout=slave,
        stderr=slave,
        start_new_session=True,
    )
    os.close(slave)
    output = bytearray()
    screen = pyte.Screen(120, 35)
    stream = pyte.ByteStream(screen)
    started = time.monotonic()
    sent = False
    resized = False
    submitted = False
    exited = False
    try:
        while time.monotonic() - started < 40:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    chunk = os.read(master, 65536)
                    output.extend(chunk)
                    stream.feed(chunk)
                except OSError:
                    break
            elapsed = time.monotonic() - started
            if elapsed > 5 and not sent:
                if submit:
                    os.write(master, "Terminal proof 雪".encode())
                    if not resize:
                        os.write(master, b"\r")
                sent = True
            if resize and elapsed > 6 and not resized:
                assert "Terminal proof 雪" in "\n".join(screen.display), screen.display
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 130, 0, 0))
                screen.resize(40, 130)
                os.killpg(process.pid, signal.SIGWINCH)
                os.write(master, b" resized")
                resized = True
            if resize and elapsed > 7 and not submitted:
                assert "Terminal proof 雪 resized" in "\n".join(screen.display), screen.display
                os.write(master, b"\r")
                submitted = True
            if elapsed > (15 if submit else 7) and not exited:
                os.write(master, b"\x03")
                exited = True
            if elapsed > (17 if submit else 9) and process.poll() is None:
                os.write(master, b"\x03")
            if process.poll() is not None:
                break
        assert process.wait(timeout=5) == 0, bytes(output)[-2000:]
        assert b"\x1b[" in output, "native renderer did not open"
        return bytes(output)
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=10)
        os.close(master)


def main() -> None:
    assert os.environ.get("LOOPFLOW_NETWORK_ISOLATED"), "run through scripts/test_network.py"
    binary = str(Path(sys.argv[1]).resolve())
    with tempfile.TemporaryDirectory(prefix="loo504-lf-") as directory, Model(hold=0) as model:
        model.tool_commands = {}
        model.tool_on_primary = True
        root = Path(directory).resolve()
        repo = root / "repo"
        repo.mkdir()
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(
            [
                "git",
                "-C",
                str(repo),
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "Fixture",
            ],
            check=True,
        )
        (repo / ".lf").mkdir()
        (repo / ".lf/config.yaml").write_text(
            "agent: opencode:opencode/probe\ndiff: false\ndiff_files: false\npaste: false\n"
        )
        (repo / "scratch").mkdir()
        (repo / "scratch/proof.md").write_text("CONTEXT_INITIAL_雪\n")
        environment = isolated_environment(root)
        environment.update(LF_BIN=binary, LF_USER_NAME="Native Fixture")
        environment["OPENCODE_CONFIG_CONTENT"] = json.dumps(
            {
                "model": "opencode/probe",
                "providers": {
                    "opencode": {
                        "package": "@ai-sdk/openai-compatible",
                        "settings": {"baseURL": f"http://127.0.0.1:{model.server_port}/v1"},
                        "models": {"probe": {"limit": {"context": 200000, "output": 1000}}},
                    }
                },
            }
        )

        if "--resize" in sys.argv[2:]:
            _terminal(binary, ["-a", "opencode"], repo, environment, True, resize=True)
            assert any(
                "Terminal proof 雪 resized" in json.dumps(r, ensure_ascii=False)
                for r in model.requests
            )
            print(
                json.dumps(
                    {
                        "native_resize_preserves_editable_draft": True,
                        "edited_draft_reaches_model": True,
                    },
                    indent=2,
                )
            )
            return

        def run(args: list[str]) -> subprocess.CompletedProcess:
            result = subprocess.run(
                [binary, *args],
                cwd=repo,
                env=environment,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=90,
            )
            if result.returncode:
                print(result.stderr.decode(errors="replace"), file=sys.stderr)
                for log in Path(environment["LF_HOME"]).rglob("opencode.stderr.log"):
                    print(log.read_text(errors="replace")[-4000:], file=sys.stderr)
            return result

        assert run(["session", "list", "--json"]).returncode == 0
        # Seed only this disposable ledger, as the Rust launch fixtures do. A
        # Wave conversation stays open after a headless provider turn finishes.
        with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
            db.execute(
                "INSERT INTO waves(id,name,repo,created_at) "
                "VALUES('0123456789abcdef0123456789abcdef','fixture',?,1)",
                (str(repo),),
            )
        if "--routing-and-failure" in sys.argv[2:]:
            model.tool_on_primary = False
            _routing_and_failure(run, model, repo, environment)
            _provider_death(binary, repo, environment, model)
            return
        first = run(["-b", "--wave", "fixture", ":", "Use the native shell tool then answer. 雪"])
        assert first.returncode == 0, first.stdout.decode(errors="replace")
        with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
            session, native = db.execute(
                "SELECT id, provider_thread FROM agent_sessions ORDER BY created_at DESC LIMIT 1"
            ).fetchone()
        assert native.startswith("ses_")
        assert len(model.requests) >= 2
        assert any("native-tool-proof" in json.dumps(r) for r in model.requests), json.dumps(
            model.requests
        )[-6000:]
        if not any(
            "CONTEXT_INITIAL_雪" in json.dumps(r, ensure_ascii=False) for r in model.requests
        ):
            for log in Path(environment["LF_HOME"]).rglob("opencode.stderr.log"):
                print(log.read_text(errors="replace")[-5000:], file=sys.stderr)
            print(
                json.dumps(
                    [
                        {
                            "roles": [m.get("role") for m in r["messages"]],
                            "system": [m for m in r["messages"] if m.get("role") == "system"],
                        }
                        for r in model.requests
                    ],
                    ensure_ascii=False,
                )[-10000:],
                file=sys.stderr,
            )
            raise AssertionError("initial context absent")
        (repo / "scratch/proof.md").write_text("CONTEXT_REFRESHED_雪\n")
        before = len(model.requests)
        resumed = run(["-b", "session", "connect", session, "--message", "Continue explicitly. 🌊"])
        assert resumed.returncode == 0
        assert len(model.requests) > before
        assert any(
            "CONTEXT_REFRESHED_雪" in json.dumps(r, ensure_ascii=False)
            for r in model.requests[before:]
        )
        with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
            assert (
                db.execute(
                    "SELECT provider_thread FROM agent_sessions WHERE id=?", (session,)
                ).fetchone()[0]
                == native
            )
        before = len(model.requests)
        _terminal(binary, ["-a", "opencode"], repo, environment, False)
        assert len(model.requests) > before, "bare lf launch did not run its default skill"
        with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
            terminal_session, terminal_native = db.execute(
                "SELECT id,provider_thread FROM agent_sessions "
                "WHERE interactive=1 ORDER BY created_at DESC LIMIT 1"
            ).fetchone()
        before = len(model.requests)
        _terminal(binary, ["session", "connect", terminal_session], repo, environment, False)
        assert len(model.requests) == before, "quitting the empty resumed composer ran a model"
        _terminal(binary, ["session", "connect", terminal_session], repo, environment, True)
        assert len(model.requests) > before, "native terminal did not submit input"
        with sqlite3.connect(Path(environment["LF_HOME"]) / "loopflow.db") as db:
            assert (
                db.execute(
                    "SELECT provider_thread FROM agent_sessions WHERE id=?", (terminal_session,)
                ).fetchone()[0]
                == terminal_native
            )
        large = "Unicode admission héllo 🌊\n" * 10000
        (repo / ".lf/skills").mkdir()
        (repo / ".lf/skills/large-proof.md").write_text(large + "\nLARGE_PROMPT_END\n")
        before = len(model.requests)
        assert run(["-b", "-a", "opencode:opencode/probe", "skill", "large-proof"]).returncode == 0
        assert any("LARGE_PROMPT_END" in json.dumps(r) for r in model.requests[before:])
        (repo / ".lf/flows").mkdir()
        (repo / ".lf/skills/flow-proof.md").write_text("Reply with the flow proof answer.")
        (repo / ".lf/flows/native-proof.yaml").write_text("- flow-proof\n")
        assert run(["-b", "-a", "opencode:opencode/probe", "flow", "native-proof"]).returncode == 0
        print(
            json.dumps(
                {
                    "large_unicode_skill": True,
                    "native_flow": True,
                    "terminal_quit_before_submit": True,
                    "terminal_resume_input": True,
                    "fresh_tool_turn": True,
                    "completed_resume": True,
                    "context_refresh": True,
                    "native_identity_preserved": True,
                    "model_requests": len(model.requests),
                },
                indent=2,
            )
        )


if __name__ == "__main__":
    main()
