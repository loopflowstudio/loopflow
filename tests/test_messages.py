"""Tests for message generation and parsing."""

from loopflow.lf.messages import _extract_json_payload


def test_extract_json_direct():
    """Direct JSON without any wrapper."""
    text = '{"title": "test", "body": "content"}'
    result = _extract_json_payload(text)
    assert result == {"title": "test", "body": "content"}


def test_extract_json_with_code_fence():
    """JSON inside ```json code fence."""
    text = """Here's the JSON:
```json
{"title": "test", "body": "content"}
```"""
    result = _extract_json_payload(text)
    assert result == {"title": "test", "body": "content"}


def test_extract_json_ignores_placeholders_before_fence():
    """Placeholders like {name} before the JSON fence should be ignored."""
    # Use raw string to preserve escaped newlines as Claude would output them
    text = r"""Looking at the diff:

1. New function `load_goal()` that loads from `.lf/goals/{name}.md`
2. Updates to `design.md`

```json
{"title": "folders: add goal loading", "body": "## Summary\n\nGoal loading works."}
```"""
    result = _extract_json_payload(text)
    assert result is not None
    assert result["title"] == "folders: add goal loading"


def test_extract_json_without_fence_uses_first_brace():
    """Without a fence, finds the first { character."""
    text = 'Some text {"title": "test", "body": "content"} more text'
    result = _extract_json_payload(text)
    assert result == {"title": "test", "body": "content"}


def test_extract_json_empty():
    """Empty or whitespace-only returns None."""
    assert _extract_json_payload("") is None
    assert _extract_json_payload("   ") is None


def test_extract_json_no_braces():
    """No braces returns None."""
    assert _extract_json_payload("just plain text") is None


def test_extract_json_invalid():
    """Invalid JSON returns None."""
    assert _extract_json_payload("{not valid json}") is None
