"""Disposable homes and private server ownership shared by the native probes."""

import base64
import json
import os
import secrets
import select
import signal
import subprocess
from pathlib import Path


def isolated_environment(root: Path) -> dict[str, str]:
    for name in ("home", "tmp"):
        (root / name).mkdir()
    return {
        "PATH": os.environ["PATH"],
        "HOME": str(root / "home"),
        "TMPDIR": str(root / "tmp"),
        "LF_HOME": str(root / "lf"),
        "OPENCODE_TEST_HOME": str(root / "home"),
        "OPENCODE_DISABLE_MODELS_FETCH": "1",
        "OPENCODE_CONFIG_PROJECT_DISABLE": "1",
        "OPENCODE_DISABLE_CLAUDE_CODE": "1",
        **{
            f"XDG_{name}_HOME": str(root / name.lower())
            for name in ("DATA", "CACHE", "CONFIG", "STATE")
        },
    }


class NativeServer:
    def __init__(
        self, executable: str, repo: Path, environment: dict[str, str], *, managed: bool = False
    ) -> None:
        self.executable = executable
        self.repo = repo
        self.environment = environment
        self.managed = managed
        self.process: subprocess.Popen | None = None

    def __enter__(self) -> "NativeServer":
        self.start()
        return self

    def __exit__(self, *args: object) -> None:
        self.stop()

    def start(self) -> None:
        # Each process gets a new credential; the persistent fixture home survives restart.
        password = secrets.token_urlsafe(32)
        self.authorization = "Basic " + base64.b64encode(f"opencode:{password}".encode()).decode()
        environment = dict(self.environment)
        if self.managed:
            # Only fixture-owned config is written; no existing service or account is read.
            config = Path(environment["XDG_CONFIG_HOME"]) / "opencode" / "service.json"
            config.parent.mkdir(parents=True, exist_ok=True)
            config.touch(mode=0o600, exist_ok=True)
            config.write_text(json.dumps({"password": password}))
        else:
            environment["OPENCODE_PASSWORD"] = password
        self.process = subprocess.Popen(
            [
                self.executable,
                "serve",
                "--service" if self.managed else "--stdio",
                "--hostname",
                "127.0.0.1",
                "--port",
                "0",
            ],
            cwd=self.repo,
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            assert select.select([self.process.stdout], [], [], 30)[0], "no server endpoint"
            line = self.process.stdout.readline().decode().strip()
            if self.managed:
                assert line.startswith("server listening on "), "missing managed endpoint"
                self.endpoint = line.removeprefix("server listening on ").rstrip("/")
            else:
                self.endpoint = json.loads(line)["url"].rstrip("/")
        except BaseException:
            self.stop()
            raise

    def stop(self) -> None:
        process, self.process = self.process, None
        if process is None:
            return
        process.stdin.close()
        if self.managed and process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            # A forced stop is a failed probe, never evidence of graceful shutdown.
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            raise
        finally:
            process.stdout.close()
