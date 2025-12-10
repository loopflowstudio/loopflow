"""Context gathering for LLM sessions."""

from pathlib import Path

from loopflow.files import gather_docs, gather_files, format_files


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


def gather_task(repo_root: Path, name: str) -> str | None:
    """Gather task file content from .lf/."""
    lf_dir = repo_root / ".lf"
    for ext in [".lf", ".md", ".txt", ""]:
        content = _read_file_if_exists(lf_dir / f"{name}{ext}")
        if content:
            return content
    return None


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


def build_prompt(
    repo_root: Path,
    task: str,
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

    # Task argument (the primary input to the task)
    if arg:
        arg_result = gather_arg(repo_root, arg)
        if arg_result:
            rel_path, content = arg_result
            parts.append(f"Task input.\n\n<lf:arg path=\"{rel_path}\">\n{content}\n</lf:arg>")

    # Task definition
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
