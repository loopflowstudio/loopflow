from pathlib import Path

import pytest
from codex_request_mapping import (
    _assess_requests,
    _invocation_receipts,
    _last_user_texts,
    _skill_paths,
)


@pytest.mark.parametrize(
    "mode",
    ["catalog", "original", "alias", "redelivery", "boundary", "boundary-race", "queue-race"],
)
def test_missing_requests_report_failed_evidence(mode: str) -> None:
    observation = _assess_requests([], mode, Path("/audit/SKILL.md"), Path("/snapshot"), "m", "c")
    assert observation["requests"] == 0
    assert not any(observation["checks"].values())


def test_recovery_does_not_infer_capture_from_repeated_text_or_assistant_echo():
    thread = {
        "turns": [
            {
                "id": "turn",
                "items": [
                    {"type": "userMessage", "clientId": "other", "content": [{"text": "capture"}]},
                    {"type": "agentMessage", "clientId": "capture", "text": "capture"},
                    {"type": "userMessage", "content": [{"text": "capture"}]},
                ],
            }
        ]
    }
    assert _invocation_receipts(thread, "capture") == []


def test_duplicate_native_receipts_remain_ambiguous_even_on_one_turn():
    message = {"type": "userMessage", "clientId": "capture", "content": []}
    thread = {"turns": [{"id": "turn", "items": [message, dict(message)]}]}
    assert len(_invocation_receipts(thread, "capture")) == 2


def test_interrupted_input_is_retained_without_becoming_success():
    message = {"type": "userMessage", "clientId": "capture", "content": []}
    turn = {"id": "turn", "status": "interrupted", "items": [message]}
    assert _invocation_receipts({"turns": [turn]}, "capture") == [(turn, message)]


def test_replayed_skill_history_is_not_another_expansion():
    request = {
        "input": [
            {"role": "user", "content": [{"text": "<skill>\n<path>/audit/SKILL.md</path>"}]},
            {"role": "assistant", "content": [{"text": "done"}]},
            {"role": "user", "content": [{"text": "the preserved draft"}]},
        ]
    }
    assert _skill_paths(request) == ["/audit/SKILL.md"]
    assert _last_user_texts(request) == ["the preserved draft"]
    observation = _assess_requests(
        [request] * 5, "queue-race", Path("/audit/SKILL.md"), Path("/snapshot"), "m", "c"
    )
    assert observation["checks"]["expected_conversation_requests"]
    assert not observation["checks"]["native_expansion"]
