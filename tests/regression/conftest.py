"""Shared fixtures for the regression tier.

Regression tests reuse the hermetic `LfdRuntime` from the e2e harness but are
isolated into their own directory so the nightly job can target them without
pulling in the faster per-PR e2e suite.
"""

from __future__ import annotations

from collections.abc import Iterator

import pytest

from scripts.lib.api_harness import ApiClient
from scripts.lib.lfd_runtime import LfdRuntime


@pytest.fixture
def lfd_runtime() -> Iterator[LfdRuntime]:
    """Fresh lfd per test — state isolation matters more than startup cost here."""
    with LfdRuntime() as runtime:
        yield runtime


@pytest.fixture
def api_client(lfd_runtime: LfdRuntime) -> Iterator[ApiClient]:
    with ApiClient(base_url=lfd_runtime.base_url, token=lfd_runtime.token) as client:
        yield client
