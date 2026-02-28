from __future__ import annotations

from collections.abc import Iterator

import pytest
from loopflow.client import Client

from scripts.lib.api_harness import ApiClient
from scripts.lib.lfd_runtime import LfdRuntime


@pytest.fixture(scope="session")
def lfd_runtime() -> Iterator[LfdRuntime]:
    with LfdRuntime() as runtime:
        yield runtime


@pytest.fixture(scope="session")
def api_client(lfd_runtime: LfdRuntime) -> Iterator[ApiClient]:
    with ApiClient(base_url=lfd_runtime.base_url, token=lfd_runtime.token) as client:
        yield client


@pytest.fixture(scope="session")
def lf_client(lfd_runtime: LfdRuntime) -> Iterator[Client]:
    client = Client(base_url=lfd_runtime.base_url, token=lfd_runtime.token)
    yield client
    client.close()
