from pathlib import Path

from continuity import _claude_receipts, _codex_receipts, _input_receipt


def test_codex_receipt_tolerates_provider_metadata_but_not_changed_arguments():
    expected = {"type": "text", "text": "$audit alpha"}
    assert _input_receipt(expected, [dict(expected, text_elements=[])])
    assert not _input_receipt(expected, [{"type": "text", "text": "$audit alpha context"}])


def test_claude_answer_cannot_supply_native_receipts():
    skill = Path("/fixtures/audit/SKILL.md")
    history = [
        {
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "Base directory for this skill: /fixtures/audit\n|alpha|context",
                    },
                ]
            },
        }
    ]
    receipt = _claude_receipts(history, skill, "alpha", "context")
    assert not receipt["selected_source"]
    assert not receipt["separate_context"]


def test_claude_receipts_preserve_context_role_and_selected_source():
    skill = Path("/fixtures/audit/SKILL.md")
    history = [
        {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "Base directory for this skill: /fixtures/audit\n|alpha|",
                    },
                ]
            },
        },
        {
            "type": "attachment",
            "uuid": "hook-one",
            "renderedRole": "system",
            "attachment": {"type": "hook_additional_context", "content": ["context"]},
        },
    ]
    receipt = _claude_receipts(history, skill, "alpha", "context")
    assert receipt == {
        "selected_source": True,
        "separate_context": True,
        "context_roles": ["system"],
        "context_attachment_ids": ["hook-one"],
    }
    assert not _claude_receipts(history, Path("/other/SKILL.md"), "alpha", "context")[
        "selected_source"
    ]


def test_codex_expansion_must_belong_to_the_current_turn(tmp_path):
    skill = tmp_path / "SKILL.md"
    skill.write_text("Return the fixture marker.\n")
    inputs = [
        {"type": "text", "text": "$audit alpha"},
        {"type": "skill", "name": "audit", "path": str(skill)},
        {"type": "text", "text": "context"},
    ]
    history = [
        {
            "type": "response_item",
            "payload": {
                "role": "user",
                "internal_chat_message_metadata_passthrough": {"turn_id": "first"},
                "content": [
                    {"type": "input_text", "text": text}
                    for text in [
                        "$audit alpha",
                        "context",
                        f"<skill><path>{skill}</path>{skill.read_text()}</skill>",
                    ]
                ],
            },
        }
    ]
    assert all(_codex_receipts(history, "first", skill, inputs).values())
    assert not any(_codex_receipts(history, "second", skill, inputs).values())
    history[0]["payload"]["role"] = "assistant"
    assert not any(_codex_receipts(history, "first", skill, inputs).values())
