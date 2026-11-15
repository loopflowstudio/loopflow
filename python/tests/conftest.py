"""Keep an observed outcome with pytest's disposable temporary evidence."""

import warnings

import pytest
from cleanup_evidence import record_outcome


@pytest.hookimpl(trylast=True)
def pytest_sessionfinish(session: pytest.Session, exitstatus: int) -> None:
    factory = getattr(session.config, "_tmp_path_factory", None)
    root = getattr(factory, "_basetemp", None)
    if root is not None:
        try:
            record_outcome(root, passed=exitstatus == 0)
        except OSError as exc:
            warnings.warn(f"Temporary evidence outcome remains unknown: {exc}", stacklevel=1)
