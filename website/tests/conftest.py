"""Pytest configuration for website tests."""

import os
import socket
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path
from typing import Generator, TextIO

import pytest
from playwright.sync_api import Page


def pytest_configure(config):
    """Configure pytest markers."""
    config.addinivalue_line(
        "markers", "slow: marks tests as slow (deselect with '-m \"not slow\"')"
    )


@pytest.fixture(scope="session")
def browser_context_args(browser_context_args):
    """Configure browser context for accessibility testing."""
    return {
        **browser_context_args,
        "viewport": {"width": 1280, "height": 720},
        "reduced_motion": "reduce",
    }


@pytest.fixture(scope="session")
def server() -> Generator[str, None, None]:
    """Start the website server for browser tests."""
    website_dir = Path(__file__).parent.parent
    subprocess.run(
        ["uv", "run", "python", "dev.py", "sync-docs"],
        cwd=website_dir,
        check=True,
    )

    port = _find_free_port()
    env = {**os.environ, "PORT": str(port)}
    with tempfile.TemporaryFile(mode="w+") as server_log:
        process = subprocess.Popen(
            ["uv", "run", "python", "main.py"],
            cwd=website_dir,
            env=env,
            stdout=server_log,
            stderr=subprocess.STDOUT,
            text=True,
        )
        base_url = f"http://127.0.0.1:{port}"
        try:
            _wait_for_server(base_url, process, server_log)
            yield base_url
        finally:
            process.terminate()
            process.wait(timeout=10)


@pytest.fixture(scope="session")
def base_url(server: str) -> str:
    """Return the test server URL."""
    return server


@pytest.fixture
def homepage(page: Page, base_url: str):
    page.goto(base_url)
    return page


def _find_free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _wait_for_server(
    base_url: str, process: subprocess.Popen[str], server_log: TextIO
) -> None:
    deadline = time.time() + 15
    while time.time() < deadline:
        if process.poll() is not None:
            server_log.seek(0)
            raise RuntimeError(
                f"Website server exited early.\noutput:\n{server_log.read()}"
            )
        try:
            with urllib.request.urlopen(base_url, timeout=1) as response:
                if response.status == 200:
                    return
        except OSError:
            time.sleep(0.25)

    process.terminate()
    process.wait(timeout=10)
    server_log.seek(0)
    raise RuntimeError(
        f"Website server did not start at {base_url}.\noutput:\n{server_log.read()}"
    )
