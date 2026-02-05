"""Tests for git_hooks module."""

import os
import subprocess
import tempfile
from pathlib import Path

from loopflow.lfd.git_hooks import (
    HOOK_END_MARKER,
    HOOK_EVENTS,
    HOOK_MARKER,
    _get_hooks_dir,
    _has_our_hooks,
    _remove_our_hooks,
    hooks_status,
    install_hooks,
    uninstall_hooks,
)


def _init_git_repo(path: Path) -> Path:
    """Initialize a git repo at path and return it."""
    subprocess.run(["git", "init"], cwd=path, capture_output=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=path, capture_output=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=path, capture_output=True)
    return path


def test_install_hooks_creates_all_hooks():
    """install_hooks creates all expected hook files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))

        installed = install_hooks(repo)

        assert set(installed) == set(HOOK_EVENTS.keys())
        for hook_name in HOOK_EVENTS:
            hook_path = repo / ".git" / "hooks" / hook_name
            assert hook_path.exists()
            assert os.access(hook_path, os.X_OK)  # Executable


def test_install_hooks_is_idempotent():
    """install_hooks doesn't duplicate if already installed."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))

        # Install twice
        first = install_hooks(repo)
        second = install_hooks(repo)

        assert len(first) == len(HOOK_EVENTS)
        assert len(second) == 0  # Already installed

        # Verify no duplication
        hook_path = repo / ".git" / "hooks" / "post-commit"
        content = hook_path.read_text()
        assert content.count(HOOK_MARKER) == 1


def test_install_hooks_preserves_existing():
    """install_hooks appends to existing hooks."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))
        hooks_dir = repo / ".git" / "hooks"
        hooks_dir.mkdir(parents=True, exist_ok=True)

        # Create existing hook
        existing_hook = hooks_dir / "post-commit"
        existing_hook.write_text("#!/bin/bash\necho 'existing'\n")
        existing_hook.chmod(0o755)

        install_hooks(repo)

        content = existing_hook.read_text()
        assert "echo 'existing'" in content
        assert HOOK_MARKER in content


def test_uninstall_hooks_removes_our_code():
    """uninstall_hooks removes our code but preserves other code."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))
        hooks_dir = repo / ".git" / "hooks"
        hooks_dir.mkdir(parents=True, exist_ok=True)

        # Create existing hook
        existing_hook = hooks_dir / "post-commit"
        existing_hook.write_text("#!/bin/bash\necho 'existing'\n")
        existing_hook.chmod(0o755)

        # Install our hooks
        install_hooks(repo)

        # Uninstall
        removed = uninstall_hooks(repo)

        assert "post-commit" in removed
        content = existing_hook.read_text()
        assert "echo 'existing'" in content
        assert HOOK_MARKER not in content


def test_uninstall_hooks_removes_empty_files():
    """uninstall_hooks removes hook files that only had our code."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))

        # Install hooks (creates new files)
        install_hooks(repo)

        # Uninstall
        uninstall_hooks(repo)

        # Files should be removed (only had our code)
        for hook_name in HOOK_EVENTS:
            hook_path = repo / ".git" / "hooks" / hook_name
            assert not hook_path.exists()


def test_hooks_status_shows_installed():
    """hooks_status returns correct status."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))

        # Initially not installed
        status = hooks_status(repo)
        assert all(not v for v in status.values())

        # Install
        install_hooks(repo)

        # Now installed
        status = hooks_status(repo)
        assert all(v for v in status.values())


def test_has_our_hooks_detects_marker():
    """_has_our_hooks correctly detects our marker."""
    with tempfile.TemporaryDirectory() as tmpdir:
        hook_path = Path(tmpdir) / "hook"

        # No file
        assert not _has_our_hooks(hook_path)

        # File without marker
        hook_path.write_text("#!/bin/bash\necho 'other'\n")
        assert not _has_our_hooks(hook_path)

        # File with marker
        hook_path.write_text(f"#!/bin/bash\n{HOOK_MARKER}\necho 'ours'\n")
        assert _has_our_hooks(hook_path)


def test_remove_our_hooks_preserves_other_code():
    """_remove_our_hooks preserves code outside our markers."""
    content = f"""#!/bin/bash
echo 'before'
{HOOK_MARKER}
echo 'ours'
{HOOK_END_MARKER}
echo 'after'
"""
    result = _remove_our_hooks(content)
    assert "echo 'before'" in result
    assert "echo 'after'" in result
    assert "echo 'ours'" not in result
    assert HOOK_MARKER not in result


def test_get_hooks_dir_regular_repo():
    """_get_hooks_dir returns correct path for regular repos."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))

        hooks_dir = _get_hooks_dir(repo)

        assert hooks_dir == repo / ".git" / "hooks"


def test_hook_content_has_correct_structure():
    """Installed hooks have correct shell script structure."""
    with tempfile.TemporaryDirectory() as tmpdir:
        repo = _init_git_repo(Path(tmpdir))
        install_hooks(repo)

        hook_path = repo / ".git" / "hooks" / "post-commit"
        content = hook_path.read_text()

        # Should have shebang
        assert content.startswith("#!/bin/bash")

        # Should have our markers
        assert HOOK_MARKER in content
        assert HOOK_END_MARKER in content

        # Should have the notification function
        assert "_lfd_notify" in content
        assert "lfd.sock" in content or "127.0.0.1:2486" in content
