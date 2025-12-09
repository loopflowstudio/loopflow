"""Context gathering for LLM sessions."""

from pathlib import Path


def find_repo_root(start: Path | None = None) -> Path | None:
    """Find the git repository root from the given path."""
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


def gather_readme(repo_root: Path) -> str | None:
    """Gather README content from the repository."""
    for name in ["README.md", "README.txt", "README"]:
        content = _read_file_if_exists(repo_root / name)
        if content:
            return content
    return None


def gather_style(repo_root: Path) -> str | None:
    """Gather STYLE guide content from the repository."""
    for name in ["STYLE.md", "STYLE.txt", "STYLE"]:
        content = _read_file_if_exists(repo_root / name)
        if content:
            return content
    return None


def gather_role(repo_root: Path, name: str = "default") -> str | None:
    """Gather role file content."""
    lf_dir = repo_root / ".lf" / "roles"
    for ext in [".lf", ".md", ".txt", ""]:
        content = _read_file_if_exists(lf_dir / f"{name}{ext}")
        if content:
            return content
    return None


def gather_task(repo_root: Path, name: str) -> str | None:
    """Gather task file content."""
    lf_dir = repo_root / ".lf" / "tasks"
    for ext in [".lf", ".md", ".txt", ""]:
        content = _read_file_if_exists(lf_dir / f"{name}{ext}")
        if content:
            return content
    return None


def build_prompt(
    repo_root: Path,
    task: str,
    role: str = "default",
) -> str:
    """Build the full prompt for an LLM session."""
    parts = []

    readme = gather_readme(repo_root)
    if readme:
        parts.append(f"# Project README\n\n{readme}")

    style = gather_style(repo_root)
    if style:
        parts.append(f"# Style Guide\n\n{style}")

    role_content = gather_role(repo_root, role)
    if role_content:
        parts.append(f"# Role: {role}\n\n{role_content}")

    task_content = gather_task(repo_root, task)
    if task_content:
        parts.append(f"# Task: {task}\n\n{task_content}")
    else:
        parts.append(f"# Task: {task}\n\nNo task file found for '{task}'.")

    return "\n\n---\n\n".join(parts)
