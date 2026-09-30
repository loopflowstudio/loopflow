import json
import os
import subprocess
from pathlib import Path

import pytest
import yaml

from scripts.check_ci_proof import find_merge_proof


def _run(**changes: object) -> dict[str, object]:
    return dict(
        {
            "id": 42,
            "head_sha": "exact-commit",
            "event": "merge_group",
            "path": ".github/workflows/ci.yml",
            "status": "completed",
            "conclusion": "success",
        },
        **changes,
    )


def _respond(monkeypatch: pytest.MonkeyPatch, payload: object) -> None:
    monkeypatch.setattr(
        subprocess,
        "run",
        lambda *args, **kwargs: subprocess.CompletedProcess([], 0, json.dumps(payload)),
    )


def test_exact_queue_success_supplies_proof_url(monkeypatch: pytest.MonkeyPatch) -> None:
    _respond(monkeypatch, {"workflow_runs": [_run()]})
    assert find_merge_proof("owner/repo", "exact-commit") == (
        "https://github.com/owner/repo/actions/runs/42"
    )


@pytest.mark.parametrize(
    "change",
    [
        {"head_sha": "earlier-commit"},
        {"event": "pull_request"},
        {"event": "push"},
        {"path": ".github/workflows/release.yml"},
        {"status": "in_progress"},
        {"conclusion": "failure"},
        {"conclusion": "cancelled"},
        {"id": None},
    ],
)
def test_other_or_incomplete_proof_keeps_full_matrix(
    monkeypatch: pytest.MonkeyPatch, change: dict[str, object]
) -> None:
    _respond(monkeypatch, {"workflow_runs": [_run(**change)]})
    assert find_merge_proof("owner/repo", "exact-commit") is None


@pytest.mark.parametrize("payload", [{"workflow_runs": []}, {}, {"workflow_runs": [None]}])
def test_missing_evidence_keeps_full_matrix(
    monkeypatch: pytest.MonkeyPatch, payload: object
) -> None:
    _respond(monkeypatch, payload)
    assert find_merge_proof("owner/repo", "exact-commit") is None


@pytest.mark.parametrize(
    "error", [subprocess.TimeoutExpired("gh", 20), subprocess.CalledProcessError(1, "gh")]
)
def test_unavailable_github_keeps_full_matrix(
    monkeypatch: pytest.MonkeyPatch, error: subprocess.SubprocessError
) -> None:
    def fail(*args, **kwargs) -> None:
        raise error

    monkeypatch.setattr(subprocess, "run", fail)
    assert find_merge_proof("owner/repo", "exact-commit") is None


@pytest.mark.parametrize(
    "proof,changes,expected",
    [
        ("", {}, 0),
        ("", {"PYTHON_TEST": "skipped"}, 1),
        ("https://github.com/owner/repo/actions/runs/42", {"PYTHON_TEST": "skipped"}, 0),
        ("https://github.com/owner/repo/actions/runs/42", {"RUST_TEST": "failure"}, 1),
        ("https://github.com/owner/repo/actions/runs/42", {"SWIFT_TEST": "cancelled"}, 1),
    ],
)
def test_actual_workflow_gate_requires_proof_and_successful_warming(
    proof: str, changes: dict[str, str], expected: int
) -> None:
    workflow = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
    gate = yaml.safe_load(workflow.read_text())["jobs"]["tests-result"]["steps"][0]
    env = {**os.environ, **dict.fromkeys(gate["env"], "success")}
    env.update(REUSED_PROOF=proof, **changes)
    result = subprocess.run(["bash", "-c", gate["run"]], env=env, capture_output=True, text=True)
    assert result.returncode == expected, result.stdout
