"""Guards against the `block_on_pm` panic: PM-enabled ingest used to call
`Runtime::new().block_on(...)` from inside an axum handler's async runtime,
which Tokio kills with "Cannot start a runtime from within a runtime".
axum's panic layer couldn't always turn that into a 500, so the HTTP
connection dropped mid-response and Concerto showed "Network unavailable".
"""

from __future__ import annotations

import pytest

from scripts.lib.api_harness import ApiClient
from scripts.lib.lfd_runtime import LfdRuntime

pytestmark = pytest.mark.regression


def test_pm_enabled_ingest_returns_structured_response(
    lfd_runtime: LfdRuntime, api_client: ApiClient
) -> None:
    repo = str(lfd_runtime.repo_dir)

    # PM provider at repo level + project id on the wave — enough to make
    # `wave_pm_is_enabled` return true and exercise the blocking code path.
    lf_dir = lfd_runtime.repo_dir / ".lf"
    lf_dir.mkdir(parents=True, exist_ok=True)
    (lf_dir / "config.yaml").write_text("pm:\n  provider: asana\n")

    wave_dir = lfd_runtime.repo_dir / "wave" / "designer"
    wave_dir.mkdir(parents=True, exist_ok=True)
    (wave_dir / "designer.yaml").write_text(
        "pm:\n  provider: asana\n  asana_project: '123'\n"
    )
    (wave_dir / "1-something.md").write_text("# Something\n")

    create = api_client.request(
        "POST",
        "/v0/waves",
        json={
            "repo": repo,
            "name": "designer",
            "flow": "ship-roadmap",
            "run": False,
            "status": "paused",
        },
    )
    create.raise_for_status()
    wave_id = create.json()["id"]

    response = api_client.request(
        "POST",
        f"/v0/waves/{wave_id}/run",
        json={"flow": "build", "roadmap_item": "1-something.md"},
    )

    # The exact status doesn't matter — the assertion is that we get ONE
    # back at all. Before the fix, this would return an empty reply and
    # httpx would raise a connection error.
    assert response.status_code in {200, 400, 412, 500}, (
        f"expected structured response, got {response.status_code}: {response.text}"
    )
    assert response.content, "handler must emit a response body, not drop the connection"
