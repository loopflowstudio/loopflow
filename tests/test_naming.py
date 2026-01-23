"""Tests for branch naming utilities."""

from unittest.mock import MagicMock, patch

from loopflow.lf.naming import (
    MAGICAL,
    MUSICAL,
    generate_cycle_branch,
    generate_word_pair,
    parse_branch_for_cycle,
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


def test_parse_branch_for_cycle_no_suffix():
    """Branch without magical-musical suffix returns as-is."""
    assert parse_branch_for_cycle("jack.auth.20260123_1112") == "jack.auth.20260123_1112"


def test_parse_branch_for_cycle_with_suffix():
    """Branch with magical-musical suffix strips it."""
    assert parse_branch_for_cycle("jack.auth.20260123_1112-aurora-melody") == "jack.auth.20260123_1112"


def test_parse_branch_for_cycle_with_suffix_frost_cadence():
    """Another valid suffix is stripped."""
    assert parse_branch_for_cycle("jack.auth.20260123_1112-frost-cadence") == "jack.auth.20260123_1112"


def test_parse_branch_for_cycle_invalid_suffix_not_in_list():
    """Suffix with words not in lists is preserved."""
    # Words not in MAGICAL or MUSICAL
    assert parse_branch_for_cycle("jack.auth.20260123_1112-foo-bar") == "jack.auth.20260123_1112-foo-bar"


def test_parse_branch_for_cycle_partial_match():
    """Only magical-musical pair counts as suffix."""
    # aurora is magical but "something" is not musical
    assert parse_branch_for_cycle("jack.auth-aurora-something") == "jack.auth-aurora-something"


def test_parse_branch_for_cycle_simple_branch():
    """Simple branch name without dots."""
    assert parse_branch_for_cycle("feature-branch") == "feature-branch"


def test_parse_branch_for_cycle_recursive():
    """Parsing a cycled branch returns same base."""
    base = "jack.auth.20260123_1112"
    cycled = f"{base}-aurora-melody"
    assert parse_branch_for_cycle(cycled) == base
    # Second cycle would produce different suffix but same base
    cycled2 = f"{base}-frost-cadence"
    assert parse_branch_for_cycle(cycled2) == base


def test_generate_cycle_branch_appends_suffix():
    """Cycle branch gets magical-musical suffix."""
    with patch("loopflow.lf.naming.branch_exists", return_value=False):
        result = generate_cycle_branch("jack.auth.20260123_1112", MagicMock())
    assert result.startswith("jack.auth.20260123_1112-")
    # Should have magical-musical suffix
    suffix = result.split("-", 3)[-2:]  # Last two hyphen-separated parts
    assert suffix[0] in MAGICAL
    assert suffix[1] in MUSICAL


def test_generate_cycle_branch_retries_on_collision():
    """Retries when branch already exists."""
    call_count = [0]

    def mock_exists(repo, branch):
        call_count[0] += 1
        # First 3 attempts collide, 4th succeeds
        return call_count[0] < 4

    with patch("loopflow.lf.naming.branch_exists", side_effect=mock_exists):
        result = generate_cycle_branch("test", MagicMock())

    assert call_count[0] == 4
    assert result.startswith("test-")


def test_generate_cycle_branch_raises_on_exhaustion():
    """Raises ValueError if can't find unique branch after 100 attempts."""
    with patch("loopflow.lf.naming.branch_exists", return_value=True):
        try:
            generate_cycle_branch("test", MagicMock())
            assert False, "Should have raised ValueError"
        except ValueError as e:
            assert "Could not generate unique branch" in str(e)
