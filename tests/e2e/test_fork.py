from __future__ import annotations

from collections.abc import Iterator
import os
import subprocess
from pathlib import Path

import pytest

from scripts.lib.fork_scenarios import (
    build_lfd,
    create_and_run_wave,
    ensure_agent_image,
    ensure_postgres,
    start_lfd_container_mode,
    wait_for_completion,
)


def _docker_available() -> bool:
    result = subprocess.run(["docker", "version"], capture_output=True)
    return result.returncode == 0


def _claude_credentials() -> bool:
    claude_dir = Path.home() / ".claude"
    has_oauth = claude_dir.exists() and any(claude_dir.iterdir())
    has_api_key = bool(os.environ.get("ANTHROPIC_API_KEY"))
    return has_oauth or has_api_key


pytestmark = [
    pytest.mark.e2e,
    pytest.mark.docker,
    pytest.mark.skipif(not _docker_available(), reason="Docker not available"),
    pytest.mark.skipif(not _claude_credentials(), reason="No Claude credentials"),
]


@pytest.fixture(scope="module")
def fork_infra() -> Iterator[subprocess.Popen[str]]:
    ensure_postgres()
    ensure_agent_image()
    build_lfd()
    process = start_lfd_container_mode()
    try:
        yield process
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


def test_fork_execution(fork_infra: subprocess.Popen[str]) -> None:
    create_and_run_wave("wave-reduce", "product-engineer")
    success, output = wait_for_completion(fork_infra, timeout=300)
    assert success, f"fork execution failed:\n{output[-2000:]}"
