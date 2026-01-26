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

1. New function `load_direction()` that loads from `lf/directions/{name}.md`
2. Updates to `design.md`

```json
{"title": "folders: add direction loading", "body": "## Summary\n\nDirection loading works."}
```"""
    result = _extract_json_payload(text)
    assert result is not None
    assert result["title"] == "folders: add direction loading"


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


def test_extract_json_with_actual_newlines_in_body():
    """JSON with actual newlines in string values (not escaped) should still work."""
    # This simulates Claude outputting actual newlines in the body
    text = """```json
{
"title": "test title",
"body": "## Summary

This PR does things.

## Changes

- Added feature"
}
```"""
    result = _extract_json_payload(text)
    assert result is not None
    assert result["title"] == "test title"
    assert "## Summary" in result["body"]
    assert "## Changes" in result["body"]


def test_extract_json_mixed_escaped_and_actual_newlines():
    """JSON with both escaped \\n and actual newlines should work."""
    text = """```json
{
"title": "folders: add feature",
"body": "## Try it\\n\\n```bash
mkdir -p .docs
```"
}
```"""
    result = _extract_json_payload(text)
    assert result is not None
    assert result["title"] == "folders: add feature"
