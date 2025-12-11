"""Context gathering for LLM sessions."""

import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from loopflow.files import gather_docs, gather_files, format_files


@dataclass
class PromptComponents:
    """Raw components of a prompt before assembly."""

    docs: list[tuple[Path, str]]
    diff: str | None
    task: tuple[str, str] | None  # (name, content)
    context_files: list[tuple[Path, str]]
    repo_root: Path


def find_worktree_root(start: Optional[Path] = None) -> Path | None:
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


def gather_diff(repo_root: Path, exclude: Optional[list[str]] = None) -> str | None:
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

    # Get diff against main, excluding specified patterns
    cmd = ["git", "diff", "main...HEAD"]
    if exclude:
        cmd.append("--")
        cmd.extend(f":(exclude){pattern}" for pattern in exclude)

    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None

    return result.stdout


def gather_prompt_components(
    repo_root: Path,
    task: Optional[str] = None,
    inline: Optional[str] = None,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
) -> PromptComponents:
    """Gather all prompt components without assembling them."""
    docs = gather_docs(repo_root, repo_root, exclude)
    diff = gather_diff(repo_root, exclude)

    task_result = None
    if inline:
        task_result = ("inline", inline)
    elif task:
        task_content = gather_task(repo_root, task)
        if task_content:
            task_result = (task, task_content)
        else:
            task_result = (task, f"No task file found for '{task}'.")

    context_files = gather_files(context, repo_root, exclude) if context else []

    return PromptComponents(
        docs=docs,
        diff=diff,
        task=task_result,
        context_files=context_files,
        repo_root=repo_root,
    )


def format_prompt(components: PromptComponents) -> str:
    """Format prompt components into the final prompt string."""
    parts = []

    if components.task:
        name, content = components.task
        if name == "inline":
            parts.append(f"The task.\n\n<lf:task>\n{content}\n</lf:task>")
        else:
            parts.append(f"The task.\n\n<lf:task:{name}>\n{content}\n</lf:task:{name}>")

    if components.docs:
        doc_parts = []
        for doc_path, content in components.docs:
            name = doc_path.stem
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(f"Repository documentation. Follow VOICE and STYLE carefully. May include a design doc (<branch>.md) for the current branch.\n\n<lf:docs>\n{docs_body}\n</lf:docs>")

    if components.diff:
        parts.append(f"Changes on this branch (diff against main).\n\n<lf:diff>\n{components.diff}\n</lf:diff>")

    if components.context_files:
        parts.append(format_files(components.context_files, components.repo_root))

    return "\n\n".join(parts)


def build_prompt(
    repo_root: Path,
    task: Optional[str] = None,
    inline: Optional[str] = None,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
) -> str:
    """Build the full prompt for an LLM session."""
    components = gather_prompt_components(repo_root, task, inline, context, exclude)
    return format_prompt(components)
