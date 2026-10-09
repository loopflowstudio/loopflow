from first_turn_transport import _assess


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
    assert not _assess([_request("request"), _request("request")], "request")["one_model_request"]


def test_later_or_duplicate_turn_cannot_mask_the_launch_input():
    for texts in [["request", "another turn"], ["request", "request"]]:
        request = {"input": [item for text in texts for item in _request(text)["input"]]}
        assert not _assess([request], "request")["complete_first_turn"]
