#!/usr/bin/env python3
"""Hermetic lfd runtime for API E2E suites."""

from __future__ import annotations

import os
import socket
import subprocess
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from types import TracebackType
from typing import IO, Optional

import httpx

REPO_ROOT = Path(__file__).resolve().parents[2]


@dataclass
class LfdRuntime:
    """Build and run lfd against an isolated HOME and temp git repo."""

    repo_root: Path = REPO_ROOT
    startup_timeout_seconds: float = 30.0
    build_binary: bool = True

    base_url: str = field(init=False)
    token: str = field(init=False)
    home_dir: Path = field(init=False)
    repo_dir: Path = field(init=False)
    log_path: Path = field(init=False)

    _temp_dir: Optional[tempfile.TemporaryDirectory[str]] = field(init=False, default=None)
    _log_handle: Optional[IO[str]] = field(init=False, default=None)
    _process: Optional[subprocess.Popen[str]] = field(init=False, default=None)

    def __enter__(self) -> "LfdRuntime":
        self.start()
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        self.stop()

    def start(self) -> None:
        if self._process is not None:
            raise RuntimeError("lfd runtime already started")

        if self.build_binary:
            _run_checked(["cargo", "build", "--bin", "lfd"], cwd=self.repo_root)

        self._temp_dir = tempfile.TemporaryDirectory(prefix="lfd-api-")
        temp_root = Path(self._temp_dir.name)
        self.home_dir = temp_root / "home"
        self.repo_dir = temp_root / "repo"
        self.log_path = temp_root / "lfd.log"

        self.home_dir.mkdir(parents=True, exist_ok=True)
        self._initialize_git_repo(self.repo_dir)

        port = _reserve_port()
        self.base_url = f"http://127.0.0.1:{port}"

        env = os.environ.copy()
        env["HOME"] = str(self.home_dir)
        env["LFD_HTTP_ADDR"] = f"127.0.0.1:{port}"
        env["GRPC_ENABLE_FORK_SUPPORT"] = "0"
        env["GRPC_VERBOSITY"] = "ERROR"

        self._log_handle = self.log_path.open("w", encoding="utf-8")
        lfd_bin = self.repo_root / "target" / "debug" / "lfd"
        self._process = subprocess.Popen(
            [str(lfd_bin), "serve"],
            cwd=self.repo_root,
            env=env,
            stdout=self._log_handle,
            stderr=subprocess.STDOUT,
            text=True,
        )

        try:
            self._wait_for_health()
            self.token = self._wait_for_session_token()
        except Exception:
            self.stop()
            raise

    def stop(self) -> None:
        process = self._process
        self._process = None

        if process is not None:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

        if self._log_handle is not None:
            self._log_handle.close()
            self._log_handle = None

        if self._temp_dir is not None:
            self._temp_dir.cleanup()
            self._temp_dir = None

    def logs(self) -> str:
        if not hasattr(self, "log_path") or not self.log_path.exists():
            return ""
        if self._log_handle is not None:
            self._log_handle.flush()
        return self.log_path.read_text(encoding="utf-8")

    def _wait_for_health(self) -> None:
        deadline = time.time() + self.startup_timeout_seconds
        last_error = ""

        while time.time() < deadline:
            if self._process is not None and self._process.poll() is not None:
                logs = self.logs()
                raise RuntimeError(
                    f"lfd exited before becoming healthy (exit={self._process.returncode})\n{logs}"
                )

            try:
                response = httpx.get(f"{self.base_url}/health", timeout=1.0)
                if response.status_code == 200:
                    return
                last_error = f"unexpected health status {response.status_code}: {response.text}"
            except httpx.HTTPError as exc:
                last_error = str(exc)

            time.sleep(0.2)

        raise RuntimeError(f"timed out waiting for lfd health: {last_error}\n{self.logs()}")

    def _wait_for_session_token(self) -> str:
        token_path = self.home_dir / ".lf" / "session-token"
        deadline = time.time() + self.startup_timeout_seconds

        while time.time() < deadline:
            if self._process is not None and self._process.poll() is not None:
                logs = self.logs()
                raise RuntimeError(
                    f"lfd exited before writing session token (exit={self._process.returncode})\n{logs}"
                )

            if token_path.exists():
                token = token_path.read_text(encoding="utf-8").strip()
                if token:
                    return token
            time.sleep(0.1)

        raise RuntimeError(f"timed out waiting for session token at {token_path}")

    @staticmethod
    def _initialize_git_repo(repo_dir: Path) -> None:
        repo_dir.mkdir(parents=True, exist_ok=True)
        _run_checked(["git", "init"], cwd=repo_dir)
        _run_checked(["git", "checkout", "-B", "main"], cwd=repo_dir)
        _run_checked(["git", "config", "user.name", "Loopflow"], cwd=repo_dir)
        _run_checked(["git", "config", "user.email", "loopflow@example.com"], cwd=repo_dir)
        (repo_dir / "README.md").write_text("# runtime repo\n", encoding="utf-8")
        _run_checked(["git", "add", "README.md"], cwd=repo_dir)
        _run_checked(["git", "commit", "-m", "init"], cwd=repo_dir)


def _reserve_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _run_checked(cmd: list[str], cwd: Path) -> None:
    result = subprocess.run(
        cmd,
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({' '.join(cmd)})\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
