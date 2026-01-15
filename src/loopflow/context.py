"""Context gathering for LLM sessions."""

import subprocess
from dataclasses import dataclass
from importlib import resources
from pathlib import Path
from typing import Optional

from loopflow.design import gather_design_docs
from loopflow.files import gather_docs, gather_files, format_files
from loopflow.frontmatter import TaskFile, parse_task_file
from loopflow.voices import Voice, load_voice


@dataclass
class PromptComponents:
    """Raw components of a prompt before assembly."""

    run_mode: str | None
    docs: list[tuple[Path, str]]
    diff: str | None
    diff_files: list[tuple[Path, str]]  # Includes both diff files and explicit context
    task: tuple[str, str] | None  # (name, content)
    repo_root: Path
    clipboard: str | None = None
    loopflow_doc: str | None = None  # Bundled system documentation
    voices: list[Voice] | None = None


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


def _read_file_if_named(dir_path: Path, filename: str) -> str | None:
    """Read file only if an exact name match exists in the directory."""
    if not dir_path.exists():
        return None
    for entry in dir_path.iterdir():
        if entry.is_file() and entry.name == filename:
            return entry.read_text()
    return None


def _read_clipboard() -> str | None:
    """Read text from clipboard using pbpaste."""
    result = subprocess.run(["pbpaste"], capture_output=True, text=True)
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout
    return None


def gather_task(repo_root: Path, name: str) -> TaskFile | None:
    """Gather and parse task file with frontmatter.

    Search order:
    1. .claude/commands/{name}.md (Claude Code compatible)
    2. .lf/{name}.lf
    3. .lf/{name}.md
    4. .lf/{name}.* (any other extension)
    5. .lf/{name} (bare name)

    Returns TaskFile with parsed config, or None if not found.
    """
    # Check .claude/commands first (portable format)
    claude_dir = repo_root / ".claude" / "commands"
    content = _read_file_if_named(claude_dir, f"{name}.md")
    if content:
        return parse_task_file(name, content)

    # Fall back to .lf directory
    lf_dir = repo_root / ".lf"

    # Preferred extensions first
    for ext in [".lf", ".md"]:
        content = _read_file_if_named(lf_dir, f"{name}{ext}")
        if content:
            return parse_task_file(name, content)

    # Any other extension
    if lf_dir.exists():
        for path in sorted(lf_dir.iterdir()):
            if not path.is_file():
                continue
            if not path.name.startswith(f"{name}."):
                continue
            if path.suffix in [".lf", ".md"]:
                continue
            content = path.read_text()
            if content:
                return parse_task_file(name, content)

    # Bare name (no extension)
    content = _read_file_if_named(lf_dir, name)
    if content:
        return parse_task_file(name, content)
    return None


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


def gather_diff_files(repo_root: Path) -> list[str]:
    """Return file paths touched by this branch vs main.

    Filters out deleted files (can't load those).
    Exclude patterns are applied later when files are loaded via gather_files().
    """
    # Get current branch
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    if not branch or branch == "main":
        return []

    result = subprocess.run(
        ["git", "diff", "--name-only", "main...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return []

    paths = []
    for line in result.stdout.strip().split("\n"):
        if not line:
            continue
        path = repo_root / line
        if path.exists():  # filter deleted files
            paths.append(line)
    return paths


def _load_loopflow_doc() -> str:
    """Load LOOPFLOW.md from the package."""
    return resources.files("loopflow").joinpath("LOOPFLOW.md").read_text()


def gather_prompt_components(
    repo_root: Path,
    task: Optional[str] = None,
    inline: Optional[str] = None,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    task_args: Optional[list[str]] = None,
    paste: bool = False,
    include_tests_for: Optional[list[str]] = None,
    run_mode: Optional[str] = None,
    include_loopflow_doc: bool = True,
    voices: Optional[list[str]] = None,
    include_diff: bool = False,
    include_diff_files: bool = True,
) -> PromptComponents:
    """Gather all prompt components without assembling them."""
    docs = gather_docs(repo_root, repo_root, exclude)

    # Load bundled LOOPFLOW.md (system documentation)
    loopflow_doc = _load_loopflow_doc() if include_loopflow_doc else None

    # Insert design docs before repo docs
    design_docs = gather_design_docs(repo_root)
    if design_docs:
        docs[0:0] = design_docs

    diff = gather_diff(repo_root, exclude) if include_diff else None

    task_result = None
    if inline:
        task_result = ("inline", inline)
    elif task:
        task_file = gather_task(repo_root, task)
        if task_file:
            task_content = task_file.content
            # Process task_args if provided
            if task_args:
                plain_args = []
                for arg in task_args:
                    if "=" in arg:
                        # Template substitution: {{key}} -> value
                        key, value = arg.split("=", 1)
                        task_content = task_content.replace(f"{{{{{key}}}}}", value)
                    else:
                        plain_args.append(arg)
                # Append plain args to task content
                if plain_args:
                    task_content = task_content.rstrip() + "\n\n" + " ".join(plain_args)
            task_result = (task, task_content)
        else:
            task_result = (task, f"No task file found for '{task}'.")

    context_exclude = list(exclude) if exclude else []
    if include_tests_for is not None:
        task_name = task or "inline"
        include_tests = task_name in set(include_tests_for)
        if not include_tests and "tests/**" not in context_exclude:
            context_exclude.append("tests/**")

    # Gather file paths (not content yet)
    diff_file_paths = gather_diff_files(repo_root) if include_diff_files else []
    context_paths = context or []

    # Merge: diff files first, then context paths not already in diff
    diff_set = set(diff_file_paths)
    all_file_paths = diff_file_paths + [p for p in context_paths if p not in diff_set]
    all_files = gather_files(all_file_paths, repo_root, context_exclude)

    clipboard = _read_clipboard() if paste else None

    # Load voices if specified
    loaded_voices = [load_voice(name, repo_root) for name in voices] if voices else None

    return PromptComponents(
        run_mode=run_mode,
        docs=docs,
        diff=diff,
        diff_files=all_files,
        task=task_result,
        repo_root=repo_root,
        clipboard=clipboard,
        loopflow_doc=loopflow_doc,
        voices=loaded_voices,
    )


def format_prompt(components: PromptComponents) -> str:
    """Format prompt components into the final prompt string."""
    parts = []

    if components.run_mode == "auto":
        parts.append(
            "Run mode is auto (headless). Proceed without pausing for questions. "
            "If you need clarification, make the best assumption you can and append "
            "any open questions to `.design/questions.md`."
        )

    if components.loopflow_doc:
        parts.append(f"<lf:loopflow>\n{components.loopflow_doc}\n</lf:loopflow>")

    if components.task:
        name, content = components.task
        task_tag = f"<lf:task>\n{content}\n</lf:task>" if name == "inline" else f"<lf:task:{name}>\n{content}\n</lf:task:{name}>"

        # Voices go between "The task." header and the actual task content
        if components.voices:
            if len(components.voices) == 1:
                v = components.voices[0]
                voice_section = f"<lf:voice:{v.name}>\n{v.content}\n</lf:voice:{v.name}>"
            else:
                voice_parts = [f"<lf:voice:{v.name}>\n{v.content}\n</lf:voice:{v.name}>" for v in components.voices]
                voice_section = f"<lf:voices>\n{chr(10).join(voice_parts)}\n</lf:voices>"
            parts.append(f"The task.\n\n{voice_section}\n\n{task_tag}")
        else:
            parts.append(f"The task.\n\n{task_tag}")

    if components.docs:
        doc_parts = []
        for doc_path, content in components.docs:
            name = doc_path.stem
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(
            "Repository documentation. Follow STYLE carefully. "
            "May include design artifacts under .design/.\n\n"
            f"<lf:docs>\n{docs_body}\n</lf:docs>"
        )

    if components.diff:
        parts.append(f"Changes on this branch (diff against main).\n\n<lf:diff>\n{components.diff}\n</lf:diff>")

    # diff_files now contains merged diff + context files (deduplicated at load time)
    if components.diff_files:
        parts.append(format_files(components.diff_files, components.repo_root))

    if components.clipboard:
        parts.append(f"Content from clipboard.\n\n<lf:clipboard>\n{components.clipboard}\n</lf:clipboard>")

    return "\n\n".join(parts)


def build_prompt(
    repo_root: Path,
    task: Optional[str] = None,
    inline: Optional[str] = None,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    include_tests_for: Optional[list[str]] = None,
    run_mode: Optional[str] = None,
    include_loopflow_doc: bool = True,
) -> str:
    """Build the full prompt for an LLM session."""
    components = gather_prompt_components(
        repo_root,
        task,
        inline,
        context,
        exclude,
        include_tests_for=include_tests_for,
        run_mode=run_mode,
        include_loopflow_doc=include_loopflow_doc,
    )
    return format_prompt(components)
