"""Context gathering for LLM sessions."""

import subprocess
from pathlib import Path

from loopflow.files import gather_docs, gather_files, format_files


def find_worktree_root(start: Path | None = None) -> Path | None:
    """Find the git worktree root from the given path.

    In a worktree, returns the worktree root.
    In the main repo, returns the main repo root.
    Use git.find_main_repo() to always get the main repo.
    """
    path = start or Path.cwd()
    path = path.resolve()

    while path != path.parent:
        if (path / ".git").exists():
            return path
        path = path.parent

    if (path / ".git").exists():
        return path
    return None


def _read_file_if_exists(path: Path) -> str | None:
    if path.exists() and path.is_file():
        return path.read_text()
    return None


def gather_task(repo_root: Path, name: str) -> str | None:
    """Gather task file content from .lf/.

    Priority: .lf > .md > any other extension > bare name.
    """
    lf_dir = repo_root / ".lf"

    # Preferred extensions first
    for ext in [".lf", ".md"]:
        content = _read_file_if_exists(lf_dir / f"{name}{ext}")
        if content:
            return content

    # Any other extension
    for path in sorted(lf_dir.glob(f"{name}.*")):
        if path.suffix not in [".lf", ".md"]:
            content = _read_file_if_exists(path)
            if content:
                return content

    # Bare name (no extension)
    return _read_file_if_exists(lf_dir / name)


def gather_arg(repo_root: Path, arg: str) -> tuple[str, str] | None:
    """Read the task argument file. Returns (relative_path, content) or None."""
    path = (repo_root / arg).resolve()
    if not path.exists() or not path.is_file():
        return None
    try:
        rel_path = path.relative_to(repo_root)
    except ValueError:
        rel_path = path  # Outside repo, use absolute
    return (str(rel_path), path.read_text())


def gather_diff(repo_root: Path) -> str | None:
    """Get diff against main branch. Returns None if on main or no diff."""
    # Get current branch
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    if not branch or branch == "main":
        return None

    # Get diff against main
    result = subprocess.run(
        ["git", "diff", "main...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None

    return result.stdout


def build_prompt(
    repo_root: Path,
    task: str | None = None,
    inline: str | None = None,
    arg: str | None = None,
    context: list[str] | None = None,
) -> str:
    """Build the full prompt for an LLM session."""
    parts = []

    # Gather root documentation as named sections
    root_docs = gather_docs(repo_root, repo_root)
    if root_docs:
        doc_parts = []
        for doc_path, content in root_docs:
            name = doc_path.stem  # README, STYLE, VOICE, etc.
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(f"Repository documentation. Follow VOICE and STYLE carefully.\n\n<lf:docs>\n{docs_body}\n</lf:docs>")

    # Diff against main (on feature branches only)
    diff = gather_diff(repo_root)
    if diff:
        parts.append(f"Changes on this branch (diff against main).\n\n<lf:diff>\n{diff}\n</lf:diff>")

    # Task argument (the primary input to the task)
    if arg:
        arg_result = gather_arg(repo_root, arg)
        if arg_result:
            rel_path, content = arg_result
            parts.append(f"Task input.\n\n<lf:arg path=\"{rel_path}\">\n{content}\n</lf:arg>")

    # Task definition (inline prompt or task file)
    if inline:
        parts.append(f"The task.\n\n<lf:task>\n{inline}\n</lf:task>")
    elif task:
        task_content = gather_task(repo_root, task)
        if task_content:
            parts.append(f"The task.\n\n<lf:task:{task}>\n{task_content}\n</lf:task:{task}>")
        else:
            parts.append(f"The task.\n\n<lf:task:{task}>\nNo task file found for '{task}'.\n</lf:task:{task}>")

    # Additional context files
    if context:
        gathered = gather_files(context, repo_root)
        if gathered:
            parts.append(format_files(gathered, repo_root))

    return "\n\n".join(parts)
