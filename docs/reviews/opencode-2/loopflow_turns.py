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
from pathlib import Path

from native_turns import Model
from probe_support import isolated_environment


def _terminal(
    binary: str, args: list[str], repo: Path, environment: dict[str, str], submit: bool
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
    started = time.monotonic()
    sent = False
    exited = False
    try:
        while time.monotonic() - started < 40:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    break
            elapsed = time.monotonic() - started
            if elapsed > 5 and not sent:
                if submit:
                    os.write(master, "Terminal proof 雪".encode())
                    os.write(master, b"\r")
                sent = True
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
