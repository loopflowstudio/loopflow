"""Tests for branch naming utilities."""

from unittest.mock import MagicMock, patch

from loopflow.lf.naming import (
    MAGICAL,
    MUSICAL,
    generate_next_branch,
    generate_word_pair,
    parse_branch_base,
)


def test_generate_word_pair_format():
    """Word pair has magical-musical format."""
    result = generate_word_pair()
    parts = result.split("-")
    assert len(parts) == 2
    assert parts[0] in MAGICAL
    assert parts[1] in MUSICAL


def test_generate_word_pair_randomness():
    """Multiple calls produce different results (probabilistically)."""
    results = {generate_word_pair() for _ in range(20)}
    assert len(results) > 1  # Very unlikely to get same pair 20 times


def test_parse_branch_base_no_suffix():
    """Branch without magical-musical suffix returns as-is."""
    assert parse_branch_base("my-feature") == "my-feature"


def test_parse_branch_base_with_timestamp_suffix():
    """Branch with .timestamp.word1-word2 suffix strips both."""
    result = parse_branch_base("my-feature.20260127_2204.aurora-melody")
    assert result == "my-feature"


def test_parse_branch_base_strips_main():
    """Branch ending in .main strips it."""
    assert parse_branch_base("my-feature.main") == "my-feature"


def test_parse_branch_base_invalid_suffix_not_in_list():
    """Suffix with words not in lists is preserved."""
    result = parse_branch_base("my-feature.foo-bar")
    assert result == "my-feature.foo-bar"


def test_parse_branch_base_partial_match():
    """Only magical-musical pair counts as suffix."""
    # aurora is magical but "something" is not musical
    assert parse_branch_base("my-feature.aurora-something") == "my-feature.aurora-something"


def test_parse_branch_base_simple_branch():
    """Simple branch name without dots."""
    assert parse_branch_base("feature-branch") == "feature-branch"


def test_parse_branch_base_recursive():
    """Parsing a branch with suffix returns same base."""
    base = "my-feature"
    with_suffix = f"{base}.20260127_2204.aurora-melody"
    assert parse_branch_base(with_suffix) == base
    # Different suffix still yields same base
    with_suffix2 = f"{base}.20260127_2204.frost-cadence"
    assert parse_branch_base(with_suffix2) == base


def test_generate_next_branch_appends_suffix():
    """Next branch gets .timestamp.word1-word2 suffix."""
    with patch("loopflow.lf.naming.branch_exists", return_value=False):
        result = generate_next_branch("my-feature", MagicMock())
    assert result.startswith("my-feature.")
    # Should have timestamp.word1-word2 suffix
    parts = result.split(".")
    assert len(parts) == 3  # base, timestamp, words
    timestamp = parts[1]
    assert len(timestamp) == 13  # YYYYMMDD_HHMM
    assert "_" in timestamp
    words = parts[2].split("-")
    assert len(words) == 2
    assert words[0] in MAGICAL
    assert words[1] in MUSICAL


def test_generate_next_branch_retries_on_collision():
    """Retries when branch already exists."""
    call_count = [0]

    def mock_exists(repo, branch):
        call_count[0] += 1
        # First 3 attempts collide, 4th succeeds
        return call_count[0] < 4

    with patch("loopflow.lf.naming.branch_exists", side_effect=mock_exists):
        result = generate_next_branch("test", MagicMock())

    assert call_count[0] == 4
    assert result.startswith("test.")


def test_generate_next_branch_raises_on_exhaustion():
    """Raises ValueError if can't find unique branch after 100 attempts."""
    with patch("loopflow.lf.naming.branch_exists", return_value=True):
        try:
            generate_next_branch("test", MagicMock())
            assert False, "Should have raised ValueError"
        except ValueError as e:
            assert "Could not generate unique branch" in str(e)
