import json
from pathlib import Path

import pytest

from scripts.evals.instruction_levels import load_cases, score, score_responses


@pytest.mark.parametrize("case", load_cases(), ids=lambda case: case["id"])
def test_required_work_passes_and_unrequested_enrollment_fails(case: dict) -> None:
    response = {"actions": case["required"], "rationale": "Perform the requested work"}
    assert score(case, response) == []
    assert score(case, {"actions": [], "rationale": "Skip the work"})
    extra = "select_workflow"
    assert extra not in case["allowed"]
    failures = score(case, {**response, "actions": [*case["required"], extra]})
    assert failures == ["Unrequested action: select_workflow"]


def test_wave_maintenance_requires_the_guided_case() -> None:
    cases = {case["id"]: case for case in load_cases()}
    response = {"actions": ["write_scratch", "edit_wave"], "rationale": "Reconcile memory"}
    assert score(cases["guided_memory_maintenance"], response) == []
    assert score(cases["standalone_realign"], response) == ["Unrequested action: edit_wave"]


@pytest.mark.parametrize(
    "response", [None, [], {}, {"actions": "implement"}, {"actions": [3], "rationale": "invalid"}]
)
def test_malformed_response_is_not_a_pass(response: object) -> None:
    assert score(load_cases()[0], response)


def test_missing_and_invalid_responses_remain_visible(tmp_path: Path) -> None:
    cases = load_cases()[:3]
    (tmp_path / f"{cases[0]['id']}.json").write_text(
        json.dumps(
            {
                "actions": cases[0]["required"],
                "rationale": "Do the work",
            }
        )
    )
    (tmp_path / f"{cases[1]['id']}.json").write_text("not JSON")
    results = score_responses(cases, tmp_path)
    assert [result["passed"] for result in results] == [True, False, False]
    assert all(result["failures"] for result in results[1:])
