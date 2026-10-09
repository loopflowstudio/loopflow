import json
import shlex
import subprocess
from pathlib import Path

import pytest
from first_turn_transport import _assess, _editor_command, _terminal_replies


def _request(text: str, role: str = "user") -> dict:
    return {"input": [{"role": role, "content": [{"type": "input_text", "text": text}]}]}


def test_full_first_turn_arrives_in_one_user_message():
    prompt = "skill\n\nrequest λ\n" * 20000
    assert all(_assess([_request(prompt)], prompt).values())


def test_no_request_is_not_delivery():
    assert not any(_assess([], "request").values())


def test_truncated_or_pointer_turn_is_not_delivery():
    prompt = "skill\n\nrequest"
    for text in [prompt[:-1], "Read prompt.txt", "prefix " + prompt]:
        assert not _assess([_request(text)], prompt)["complete_first_turn"]


def test_instruction_channel_is_not_first_turn():
    assert not _assess([_request("request", "developer")], "request")["complete_first_turn"]


def test_multiple_requests_do_not_prove_single_turn_transport():
    for first in ["wrong launch input", "request"]:
        checks = _assess([_request(first), _request("request")], "request")
        assert not checks["one_model_request"]
        assert checks["complete_first_turn"] is (first == "request")


def test_later_or_duplicate_turn_cannot_mask_the_launch_input():
    for texts in [["request", "another turn"], ["request", "request"]]:
        request = {"input": [item for text in texts for item in _request(text)["input"]]}
        assert not _assess([request], "request")["complete_first_turn"]


def test_native_guide_before_first_turn_is_preserved():
    request = _request("native guide")
    request["input"].extend(_request("request")["input"])
    assert all(_assess([request], "request").values())


def test_terminal_normalization_is_not_exact_delivery():
    for original, received in [
        ("skill\r\nrequest", "skill\nrequest"),
        ("literal \x1b[201~ request", "literal  request"),
        ("request \t\r\n", "request"),
    ]:
        assert not _assess([_request(received)], original)["complete_first_turn"]


@pytest.mark.parametrize("query,reply", [(b"\x1b[6n", b"\x1b[1;1R"), (b"\x1b[c", b"\x1b[?1;2c")])
def test_terminal_queries_are_answered_once_across_read_boundaries(query, reply):
    for split in range(1, len(query)):
        output = bytearray(b"old output" + query)
        start = len(output)
        output.extend(query[:split])
        assert _terminal_replies(output, start) == b""
        start = len(output)
        output.extend(query[split:] + query)
        assert _terminal_replies(output, start) == reply * 2
        assert _terminal_replies(output, len(output)) == b""


def test_external_editor_copies_bytes_without_normalizing(tmp_path: Path):
    root = tmp_path / "quoted ' editor"
    root.mkdir()
    prompt = "skill λ 🐙\r\nrequest \x1b[201~ \t\r\n".encode()
    (root / "prompt.txt").write_bytes(prompt)
    target = root / "native draft.md"
    subprocess.run([*shlex.split(_editor_command(root)), str(target)], check=True, timeout=5)
    assert target.read_bytes() == prompt
    receipt = json.loads((root / "editor.json").read_text())
    assert receipt["exact_copy"]
    assert receipt["max_argument_bytes"] < 1024
