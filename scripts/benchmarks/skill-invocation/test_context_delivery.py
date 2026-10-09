import pytest
from context_delivery import _assess


@pytest.fixture(params=["claude", "codex"])
def observation(request):
    provider = request.param
    field, role = ("messages", "user") if provider == "claude" else ("input", "developer")
    bodies = [
        {
            "instructions": "Native base instructions",
            field: [
                {"role": role, "content": f"LOO444_{phase}_CONTEXT_SessionStart"},
                {"role": "developer", "content": "LOO444_FIXED_ADDITION"},
                {"role": "user", "content": "LOO444_REPO_GUIDE"},
            ],
        }
        for phase in ["START", "FRESH"]
    ]
    events = [
        {"hook_event_name": "SessionStart", "source": source} for source in ["startup", "compact"]
    ] + [{"hook_event_name": "PostCompact"}]
    return provider, bodies, events


def test_context_refresh_preserves_native_channels(observation):
    assert all(_assess(*observation).values())


def test_instruction_copy_cannot_pass_as_conversation_only(observation):
    provider, bodies, events = observation
    bodies[-1]["instructions"] += " LOO444_FRESH_CONTEXT_SessionStart"
    assert not _assess(provider, bodies, events)["fresh_context_conversation_only"]


def test_retained_old_context_is_not_refresh(observation):
    provider, bodies, events = observation
    bodies[-1]["instructions"] += " LOO444_START_CONTEXT"
    assert not _assess(provider, bodies, events)["old_context_compacted"]


def test_resume_cannot_prove_compact_hook(observation):
    provider, bodies, events = observation
    events[1]["source"] = "resume"
    assert not _assess(provider, bodies, events)["compact_hook_ran"]


def test_post_compact_receipt_does_not_prove_delivery():
    bodies = [
        {"instructions": "Native base", "input": []},
        {
            "instructions": "Native base",
            "input": [{"role": "developer", "content": "LOO444_FRESH_CONTEXT_PostCompact"}],
        },
    ]
    checks = _assess("codex", bodies, [{"hook_event_name": "PostCompact"}])
    assert checks["post_compact_hook_ran"]
    assert not checks["post_compact_does_not_inject"]
    assert not checks["fresh_context_conversation_only"]
