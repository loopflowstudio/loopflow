"""Tests for branch naming utilities."""

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from loopflow.lf.naming import (
    MAGICAL,
    MUSICAL,
    _is_timestamp,
    _is_word_pair,
    generate_branch_name,
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


def test_is_timestamp_valid():
    """Valid timestamps are recognized."""
    assert _is_timestamp("20260202_1700")
    assert _is_timestamp("19990101_0000")


def test_is_timestamp_invalid():
    """Invalid timestamps are rejected."""
    assert not _is_timestamp("2026020_1700")  # Too short
    assert not _is_timestamp("202602021700")  # No underscore
    assert not _is_timestamp("not-a-timestamp")


def test_is_word_pair_valid():
    """Valid magical-musical pairs are recognized."""
    assert _is_word_pair("aurora-melody")
    assert _is_word_pair("vale-tempo")


def test_is_word_pair_invalid():
    """Invalid word pairs are rejected."""
    assert not _is_word_pair("foo-bar")  # Not in lists
    assert not _is_word_pair("aurora-something")  # Second word not musical
    assert not _is_word_pair("aurora")  # No dash


def test_generate_branch_name_uses_config_schema(tmp_path):
    """Branch name follows config schema."""
    config_mock = MagicMock()
    config_mock.user = "jack-heart"
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming.branch_exists", return_value=False):
            result = generate_branch_name("concerto", tmp_path)

    assert result.startswith("jack-heart.concerto.")
    parts = result.split(".")
    assert len(parts) == 4  # user, name, timestamp, words
    assert parts[0] == "jack-heart"
    assert parts[1] == "concerto"
    assert _is_timestamp(parts[2])
    assert _is_word_pair(parts[3])


def test_generate_branch_name_simple_schema(tmp_path):
    """Branch name works with simple schema."""
    config_mock = MagicMock()
    config_mock.user = None
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming.branch_exists", return_value=False):
            result = generate_branch_name("feature", tmp_path)

    assert result.startswith("feature.")
    parts = result.split(".")
    assert len(parts) == 3  # name, timestamp, words


def test_generate_branch_name_requires_user_when_in_schema(tmp_path):
    """Raises error if schema requires user but user not configured."""
    config_mock = MagicMock()
    config_mock.user = None
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with pytest.raises(ValueError, match="Config missing 'user' field"):
            generate_branch_name("concerto", tmp_path)


def test_generate_branch_name_retries_on_collision(tmp_path):
    """Retries when branch already exists."""
    config_mock = MagicMock()
    config_mock.user = "test"
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    call_count = [0]

    def mock_exists(repo, branch):
        call_count[0] += 1
        return call_count[0] < 4  # First 3 attempts collide

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming.branch_exists", side_effect=mock_exists):
            result = generate_branch_name("test", tmp_path)

    assert call_count[0] == 4
    assert result.startswith("test.test.")


def test_generate_branch_name_raises_on_exhaustion(tmp_path):
    """Raises ValueError if can't find unique branch after 100 attempts."""
    config_mock = MagicMock()
    config_mock.user = "test"
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming.branch_exists", return_value=True):
            with pytest.raises(ValueError, match="Could not generate unique branch"):
                generate_branch_name("test", tmp_path)


def test_generate_branch_name_default_schema_when_no_config(tmp_path):
    """Uses default schema when no config."""
    with patch("loopflow.lf.naming.load_config", return_value=None):
        with pytest.raises(ValueError, match="Config missing 'user' field"):
            # Default schema includes {user}, so should fail without user
            generate_branch_name("test", tmp_path)


# parse_branch_base tests


def test_parse_branch_base_full_schema(tmp_path):
    """Extracts wave name from full branch with user, timestamp, words."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="jack-heart"):
            result = parse_branch_base("jack-heart.concerto.20260202_1700.aurora-melody", tmp_path)
    assert result == "concerto"


def test_parse_branch_base_no_words(tmp_path):
    """Extracts wave name when words suffix is missing."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="jack-heart"):
            result = parse_branch_base("jack-heart.concerto.20260202_1700", tmp_path)
    assert result == "concerto"


def test_parse_branch_base_user_and_name_only(tmp_path):
    """Extracts wave name when only user and name present."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="jack-heart"):
            result = parse_branch_base("jack-heart.concerto", tmp_path)
    assert result == "concerto"


def test_parse_branch_base_name_only(tmp_path):
    """Returns name as-is when it's just the wave name."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="jack-heart"):
            result = parse_branch_base("concerto", tmp_path)
    assert result == "concerto"


def test_parse_branch_base_no_user_in_schema(tmp_path):
    """Works with schema that has no user prefix."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="anyone"):
            result = parse_branch_base("concerto.20260202_1700.aurora-melody", tmp_path)
    assert result == "concerto"


def test_parse_branch_base_dotted_name(tmp_path):
    """Handles wave names with dots."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{timestamp}.{words}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="jack-heart"):
            result = parse_branch_base("jack-heart.my.feature.20260202_1700.aurora-melody", tmp_path)
    assert result == "my.feature"


def test_parse_branch_base_ts_schema(tmp_path):
    """Works with {ts} alias for timestamp."""
    config_mock = MagicMock()
    config_mock.branch_names = MagicMock()
    config_mock.branch_names.schema_ = "{user}.{name}.{ts}"

    with patch("loopflow.lf.naming.load_config", return_value=config_mock):
        with patch("loopflow.lf.naming._get_git_username", return_value="jack-heart"):
            result = parse_branch_base("jack-heart.concerto.20260202_1700", tmp_path)
    assert result == "concerto"
