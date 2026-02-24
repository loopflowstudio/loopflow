from __future__ import annotations

from collections.abc import Iterator
import subprocess

import pytest

from scripts.lib.fork_scenarios import (
    build_lfd,
    create_and_run_wave,
    ensure_agent_image,
    ensure_postgres,
    has_claude_credentials,
    start_lfd_container_mode,
    stop_process,
    wait_for_completion,
)


def _docker_available() -> bool:
    result = subprocess.run(["docker", "version"], capture_output=True)
    return result.returncode == 0

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.docker,
    pytest.mark.skipif(not _docker_available(), reason="Docker not available"),
    pytest.mark.skipif(not has_claude_credentials(), reason="No Claude credentials"),
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
        stop_process(process)


def test_fork_execution(fork_infra: subprocess.Popen[str]) -> None:
    create_and_run_wave("wave-reduce", "product-engineer")
    success, output = wait_for_completion(fork_infra, timeout=300)
    assert success, f"fork execution failed:\n{output[-2000:]}"
