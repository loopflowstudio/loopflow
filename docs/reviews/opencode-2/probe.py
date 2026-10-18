"""Probe installed OpenCode contracts in disposable homes, without account access."""

import json
import os
import pty
import re
import select
import shutil
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

from probe_support import NativeServer, isolated_environment


def _decode_output(output: bytes | bytearray) -> str:
    return re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", output.decode(errors="replace"))


def _probe_terminal(loopflow: str, repo: Path, environment: dict[str, str]) -> dict:
    master, slave = pty.openpty()
    launch = subprocess.Popen(
        [loopflow, "-a", "opencode"],
        env=environment,
        cwd=repo,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        start_new_session=True,
    )
    os.close(slave)
    terminal = bytearray()
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            readable, _, _ = select.select([master], [], [], 0.2)
            if readable:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                terminal.extend(chunk)
            if launch.poll() is not None and not readable:
                break
        launch.wait(timeout=2)
    finally:
        if launch.poll() is None:
            os.killpg(launch.pid, signal.SIGTERM)
            launch.wait(timeout=10)
        os.close(master)
    logs = list(Path(environment["LF_HOME"]).rglob("opencode.stderr.log"))
    return {
        "exit": launch.returncode,
        "missing_identity_error": b"did not report a resumable session" in terminal,
        "uppercase_cause_retained": any(
            b'Invalid value for flag --log-level: "INFO"' in log.read_bytes() for log in logs
        ),
        "cause_overlaid_on_terminal": b"Invalid value for flag" in terminal,
        "output": _decode_output(terminal)[-2000:],
    }


def _probe_server(executable: str, repo: Path, environment: dict[str, str]) -> dict:
    report = {}
    environment["OPENCODE_CONFIG_CONTENT"] = json.dumps(
        {
            "permission": {"*": "allow", "external_directory": "deny"},
            "plugin": [],
        }
    )
    server = NativeServer(executable, repo, environment)
    with server:

        def _request(
            path: str, payload: dict | None = None, authenticated: bool = True
        ) -> tuple[int, dict | list | str]:
            headers = {"Content-Type": "application/json"}
            if authenticated:
                headers["Authorization"] = server.authorization
            request = urllib.request.Request(
                server.endpoint + path,
                data=json.dumps(payload).encode() if payload is not None else None,
                headers=headers,
            )
            try:
                with urllib.request.urlopen(request, timeout=15) as response:
                    body = response.read()
                    try:
                        value = json.loads(body)
                    except json.JSONDecodeError:
                        value = "non-JSON response"
                    return response.status, value
            except urllib.error.HTTPError as error:
                return error.code, "HTTP error"

        for path in ["/", "/event", "/session", "/api/info", "/api/session"]:
            status, body = _request(path, {} if path == "/session" else None)
            report[path] = {"status": status, "json": isinstance(body, dict)}
        report["unauthenticated_server"] = {"status": _request("/api/info", authenticated=False)[0]}
        status, config = _request("/api/config")
        report["legacy_configuration"] = {
            "status": status,
            "documents": config,
        }
        status, created = _request("/api/session", {"location": {"directory": str(repo)}})
        assert status == 200 and isinstance(created, dict), (status, created)
        session = created["data"]["id"]
        report["create"] = {
            "status": status,
            "envelope": list(created),
            "native_id": session.startswith("ses_"),
        }
        report["resume_read"] = {"status": _request(f"/api/session/{session}")[0]}
        report["old_prompt"] = {
            "status": _request(
                f"/session/{session}/prompt_async",
                {"parts": [{"type": "text", "text": "probe"}]},
            )[0]
        }
        # Durable admission without starting a model or accessing credentials.
        text = "héllo 🌊\n" * 20000
        status, admitted = _request(
            f"/api/session/{session}/prompt", {"text": text, "resume": False}
        )
        report["queued_unicode_input"] = {
            "status": status,
            "exact": isinstance(admitted, dict)
            and admitted.get("data", {}).get("payload", {}).get("text") == text,
            "bytes": len(text.encode()),
            "fields": list(admitted.get("data", {})) if isinstance(admitted, dict) else [],
        }
    return report


def main() -> None:
    assert os.environ.get("LOOPFLOW_NETWORK_ISOLATED"), "run through scripts/test_network.py"
    executable = shutil.which("opencode")
    assert executable
    with tempfile.TemporaryDirectory(prefix="loo504-probe-") as directory:
        root = Path(directory)
        repo = root / "repo"
        repo.mkdir()
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(
            [
                "git",
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-C",
                str(repo),
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "Fixture",
            ],
            check=True,
        )
        environment = isolated_environment(root)
        report = {}
        loopflow = shutil.which("lf")
        assert loopflow
        report["loopflow_version"] = (
            subprocess.run(
                [loopflow, "--version"],
                env=environment,
                capture_output=True,
                check=True,
            )
            .stdout.decode()
            .strip()
        )
        report["lf_terminal"] = _probe_terminal(loopflow, repo, environment)
        for label, args in {
            "version": ["--version"],
            "uppercase_log_level": [
                "--log-level",
                "INFO",
                "session",
                "export",
                "ses_probe",
                "--standalone",
            ],
            "lowercase_log_level": ["--log-level", "info", "--help"],
            "terminal_model_flag": ["--model", "opencode/glm-5.2", "--standalone"],
            "old_export_command": ["export", "ses_probe"],
        }.items():
            result = subprocess.run(
                [executable, *args],
                env=environment,
                cwd=repo,
                capture_output=True,
                timeout=20,
            )
            report[label] = {"exit": result.returncode}
            if label == "version":
                report[label]["value"] = result.stdout.decode().strip()
            elif result.returncode:
                report[label]["error"] = _decode_output(result.stderr)[:1200].strip()

        report.update(_probe_server(executable, repo, environment))
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
