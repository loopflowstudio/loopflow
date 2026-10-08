from pathlib import Path

import pytest
from request_mapping import _assess


@pytest.mark.parametrize("role", ["system", "assistant", "user", None])
def test_context_must_reach_the_model_as_user_input(role):
    skill = Path("/fixture/audit/SKILL.md")
    request = {
        "messages": [
            {
                "role": "user",
                "content": f"Base directory for this skill: {skill.parent}\nmarker|alpha|",
            }
        ],
        "renderedRole": "user",
    }
    if role == "system":
        request["system"] = [{"type": "text", "text": "context"}]
    elif role:
        request["messages"].append({"role": role, "content": "context"})
    facts = _assess([request], skill, "marker", "context", "alpha")
    assert facts["context_user_only"] is (role == "user")
    assert facts["selected_source"]
    assert facts["expanded_argument"]


def test_missing_requests_cannot_pass_vacuously():
    assert not any(
        _assess([], Path("/fixture/audit/SKILL.md"), "marker", "context", "alpha").values()
    )


def test_context_must_not_be_injected_in_both_user_and_system():
    request = {
        "system": "context",
        "messages": [{"role": "user", "content": "marker|alpha| context"}],
    }
    facts = _assess([request], Path("/fixture/audit/SKILL.md"), "marker", "context", "alpha")
    assert not facts["context_user_only"]
    assert not facts["selected_source"]


def test_source_argument_and_extra_turn_regressions_remain_visible():
    request = {
        "messages": [
            {
                "role": "user",
                "content": "Base directory for this skill: /other\nmarker|alpha context|",
            }
        ]
    }
    facts = _assess(
        [request, request], Path("/fixture/audit/SKILL.md"), "marker", "context", "alpha"
    )
    assert facts["request_observed"]
    assert not facts["single_model_request"]
    assert not facts["selected_source"]
    assert not facts["expanded_argument"]


def test_assistant_echo_is_not_native_expansion():
    request = {
        "messages": [
            {
                "role": "assistant",
                "content": "Base directory for this skill: /fixture/audit\nmarker|alpha| context",
            }
        ]
    }
    facts = _assess([request], Path("/fixture/audit/SKILL.md"), "marker", "context", "alpha")
    assert not facts["context_user_only"]
    assert not facts["selected_source"]
    assert not facts["expanded_argument"]
