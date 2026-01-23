"""Git hook installation for lfd notifications.

Installs hooks that notify lfd when git operations complete.
Hooks are appended to existing hooks, not replaced.
"""

import stat
import subprocess
from pathlib import Path

HOOK_MARKER = "# lfd-hook-start"
HOOK_END_MARKER = "# lfd-hook-end"

HOOK_TEMPLATE = """
{marker}
# Notify lfd of git operations (loopflow)
_lfd_notify() {{
    local socket="$HOME/.lf/lfd.sock" event="$1" repo branch
    repo="$(git rev-parse --show-toplevel 2>/dev/null)" || return
    branch="$(git branch --show-current 2>/dev/null)"
    [ -S "$socket" ] || return
    local msg='{{"method":"notify","params":{{"event":"git.'
    msg+="$event"'","data":{{"repo":"'"$repo"'","branch":"'"$branch"'"}}}}}}'
    printf '%s\\n' "$msg" | nc -U "$socket" 2>/dev/null &
}}
_lfd_notify "{event}"
{end_marker}
"""

HOOK_EVENTS = {
    "post-commit": "commit",
    "post-checkout": "checkout",
    "post-merge": "merge",
    "post-rewrite": "rewrite",
}


def _get_hooks_dir(repo: Path) -> Path:
    """Get the hooks directory for a repo, handling worktrees."""
    git_dir = repo / ".git"
    if git_dir.is_file():
        # Worktree - .git is a file pointing to the real git dir
        content = git_dir.read_text().strip()
        if content.startswith("gitdir: "):
            git_dir = Path(content[8:])
            # For worktrees, hooks are in the main repo
            # Go from .git/worktrees/name to .git/hooks
            if "worktrees" in git_dir.parts:
                main_git = git_dir.parent.parent
                return main_git / "hooks"
    return git_dir / "hooks"


def _has_our_hooks(hook_path: Path) -> bool:
    """Check if our hook code is already installed."""
    if not hook_path.exists():
        return False
    content = hook_path.read_text()
    return HOOK_MARKER in content


def _remove_our_hooks(content: str) -> str:
    """Remove our hook code from existing hook content."""
    lines = content.split("\n")
    result = []
    in_our_section = False
    for line in lines:
        if HOOK_MARKER in line:
            in_our_section = True
            continue
        if HOOK_END_MARKER in line:
            in_our_section = False
            continue
        if not in_our_section:
            result.append(line)
    # Remove trailing empty lines
    while result and not result[-1].strip():
        result.pop()
    return "\n".join(result)


def install_hooks(repo: Path) -> list[str]:
    """Install lfd notification hooks in a repo.

    Returns list of hook names installed.
    Preserves existing hooks by appending our code.
    """
    hooks_dir = _get_hooks_dir(repo)
    hooks_dir.mkdir(parents=True, exist_ok=True)

    installed = []
    for hook_name, event in HOOK_EVENTS.items():
        hook_path = hooks_dir / hook_name
        hook_code = HOOK_TEMPLATE.format(
            marker=HOOK_MARKER,
            end_marker=HOOK_END_MARKER,
            event=event,
        )

        if _has_our_hooks(hook_path):
            continue

        if hook_path.exists():
            # Append to existing hook
            existing = hook_path.read_text()
            if not existing.endswith("\n"):
                existing += "\n"
            hook_path.write_text(existing + hook_code)
        else:
            # Create new hook
            hook_path.write_text("#!/bin/bash\n" + hook_code)

        # Ensure executable
        hook_path.chmod(hook_path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        installed.append(hook_name)

    return installed


def uninstall_hooks(repo: Path) -> list[str]:
    """Remove lfd notification hooks from a repo.

    Returns list of hook names modified.
    Preserves other hook code.
    """
    hooks_dir = _get_hooks_dir(repo)
    if not hooks_dir.exists():
        return []

    removed = []
    for hook_name in HOOK_EVENTS:
        hook_path = hooks_dir / hook_name
        if not hook_path.exists():
            continue
        if not _has_our_hooks(hook_path):
            continue

        content = hook_path.read_text()
        new_content = _remove_our_hooks(content)

        # If only shebang remains, remove the file
        if new_content.strip() in ("#!/bin/bash", "#!/bin/sh", ""):
            hook_path.unlink()
        else:
            hook_path.write_text(new_content + "\n")

        removed.append(hook_name)

    return removed


def hooks_status(repo: Path) -> dict[str, bool]:
    """Check which hooks are installed."""
    hooks_dir = _get_hooks_dir(repo)
    return {
        hook_name: _has_our_hooks(hooks_dir / hook_name) if hooks_dir.exists() else False
        for hook_name in HOOK_EVENTS
    }


def find_git_root(path: Path) -> Path | None:
    """Find the git root for a path."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return Path(result.stdout.strip())
    except Exception:
        pass
    return None
