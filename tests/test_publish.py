"""Tests for loopflow.publish module."""

import pytest

from loopflow.publish import PublishError, bump_version, get_version


def test_get_version():
    version = get_version()
    # Version should be semver format
    parts = version.split(".")
    assert len(parts) == 3
    assert all(part.isdigit() for part in parts)


def test_bump_version_patch():
    assert bump_version("1.2.3", "patch") == "1.2.4"
    assert bump_version("0.0.0", "patch") == "0.0.1"
    assert bump_version("0.6.1", "patch") == "0.6.2"


def test_bump_version_minor():
    assert bump_version("1.2.3", "minor") == "1.3.0"
    assert bump_version("0.0.9", "minor") == "0.1.0"
    assert bump_version("0.6.1", "minor") == "0.7.0"


def test_bump_version_major():
    assert bump_version("1.2.3", "major") == "2.0.0"
    assert bump_version("0.6.1", "major") == "1.0.0"


def test_bump_version_invalid_format():
    with pytest.raises(PublishError, match="Invalid version format"):
        bump_version("1.2", "patch")

    with pytest.raises(PublishError, match="Invalid version format"):
        bump_version("1.2.3.4", "patch")

    with pytest.raises(PublishError, match="Invalid version format"):
        bump_version("v1.2.3", "patch")
