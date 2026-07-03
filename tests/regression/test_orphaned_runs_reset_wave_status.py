"""Guards against stuck "running" waves after an lfd restart. Previously
`fail_orphaned_runs` marked in-flight runs as Failed on startup but
left the parent wave's `status` alone, so Concerto's "Run" and
"Ingest & build" buttons stayed disabled forever — the user had to stop
the wave manually to unstick it.
"""

from __future__ import annotations

import os
import socket
import subprocess
import tempfile
import time
from pathlib import Path

import httpx
import pytest

pytestmark = pytest.mark.regression

REPO_ROOT = Path(__file__).resolve().parents[2]


def test_orphaned_runs_reset_wave_status_on_restart() -> None:
    subprocess.run(
        ["cargo", "build", "--bin", "lfd"],
        cwd=REPO_ROOT,
        check=True,
    )
    lfd_bin = REPO_ROOT / "target" / "debug" / "lfd"

    with tempfile.TemporaryDirectory(prefix="lfd-orphan-") as tmp:
        root = Path(tmp)
        home = root / "home"
        repo_dir = root / "repo"
        db_path = root / "lfd.db"
        home.mkdir(parents=True)
        _init_git_repo(repo_dir)

        wave_id = _first_boot_and_stick_a_wave_in_running(lfd_bin, home, repo_dir, db_path)

        # Second boot: pretend lfd crashed and restarted. The orphan
        # cleanup should flip the wave back to Idle.
        wave_status = _reboot_and_read_wave_status(lfd_bin, home, db_path, wave_id)

        assert wave_status == "idle", (
            f"stuck wave — expected 'idle' after restart, got {wave_status!r}"
        )


def _first_boot_and_stick_a_wave_in_running(
    lfd_bin: Path, home: Path, repo_dir: Path, db_path: Path
) -> str:
    port = _reserve_port()
    with _running_lfd(lfd_bin, home, db_path, port):
        token = _wait_for_token(home)
        client = httpx.Client(
            base_url=f"http://127.0.0.1:{port}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10.0,
        )
        create = client.post(
            "/v0/waves",
            json={
                "repo": str(repo_dir),
                "name": "stuck",
                "flow": "ship-roadmap",
                "run": False,
                "status": "running",
            },
        )
        create.raise_for_status()
        wave_id = create.json()["id"]

        status = client.get(f"/v0/waves/{wave_id}").json()["status"]
        assert status == "running", f"setup failed: expected running, got {status}"
        client.close()
        return wave_id


def _reboot_and_read_wave_status(lfd_bin: Path, home: Path, db_path: Path, wave_id: str) -> str:
    port = _reserve_port()
    with _running_lfd(lfd_bin, home, db_path, port):
        token = _wait_for_token(home)
        client = httpx.Client(
            base_url=f"http://127.0.0.1:{port}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10.0,
        )
        try:
            return client.get(f"/v0/waves/{wave_id}").json()["status"]
        finally:
            client.close()


def _init_git_repo(repo_dir: Path) -> None:
    repo_dir.mkdir(parents=True, exist_ok=True)

    def run(*args):
        subprocess.run(args, cwd=repo_dir, check=True, capture_output=True)

    run("git", "init")
    run("git", "checkout", "-B", "main")
    run("git", "config", "user.email", "t@example.com")
    run("git", "config", "user.name", "T")
    (repo_dir / "README.md").write_text("seed")
    run("git", "add", ".")
    run("git", "commit", "-m", "init")


class _running_lfd:
    def __init__(self, lfd_bin: Path, home: Path, db_path: Path, port: int) -> None:
        self._lfd_bin = lfd_bin
        self._home = home
        self._db_path = db_path
        self._port = port
        self._process: subprocess.Popen[bytes] | None = None
        self._log: Path = home.parent / f"lfd-{port}.log"

    def __enter__(self) -> "_running_lfd":
        env = os.environ.copy()
        env["HOME"] = str(self._home)
        env["LFD_HTTP_ADDR"] = f"127.0.0.1:{self._port}"
        env["LFD_DB_PATH"] = str(self._db_path)
        log_handle = self._log.open("ab")
        self._process = subprocess.Popen(
            [str(self._lfd_bin), "serve"],
            env=env,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
        )
        _wait_for_health(f"http://127.0.0.1:{self._port}")
        return self

    def __exit__(self, *_: object) -> None:
        process = self._process
        if process is None:
            return
        process.terminate()
        try:
            process.wait(timeout=8)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def _wait_for_health(base_url: str, timeout_seconds: float = 30.0) -> None:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        try:
            if httpx.get(f"{base_url}/health", timeout=1.0).status_code == 200:
                return
        except httpx.HTTPError:
            pass
        time.sleep(0.1)
    raise RuntimeError(f"lfd never reported healthy at {base_url}")


def _wait_for_token(home: Path, timeout_seconds: float = 30.0) -> str:
    token_path = home / ".lf" / "session-token"
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if token_path.exists():
            text = token_path.read_text().strip()
            if text:
                return text
        time.sleep(0.1)
    raise RuntimeError(f"session token never appeared at {token_path}")


def _reserve_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]
