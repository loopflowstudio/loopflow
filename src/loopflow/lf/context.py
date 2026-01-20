"""Context gathering for LLM sessions."""

import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from importlib import resources
from pathlib import Path
from typing import Optional

from loopflow.lf.config import load_config
from loopflow.lf.design import gather_design_docs
from loopflow.lf.files import gather_docs, gather_files, format_files, format_image_references
from loopflow.lf.frontmatter import TaskFile, parse_task_file
from loopflow.lf.skills import discover_skill_sources, find_skill, load_skill_prompt, list_all_skills
from loopflow.lfops.summarize import is_stale, load_summary
from loopflow.lf.voices import Voice, load_voice


# Path to bundled builtin templates
_TEMPLATES_DIR = Path(__file__).parent.parent / "templates" / "commands"


@dataclass
class ClipboardContent:
    """Content from clipboard - text, image, or both."""
    text: str | None = None
    image_path: Path | None = None


@dataclass
class PromptComponents:
    """Raw components of a prompt before assembly."""

    run_mode: str | None
    docs: list[tuple[Path, str]]
    diff: str | None
    diff_files: list[tuple[Path, str]]  # Includes both diff files and explicit context
    task: tuple[str, str] | None  # (name, content)
    repo_root: Path
    clipboard: ClipboardContent | None = None
    loopflow_doc: str | None = None  # Bundled system documentation
    voices: list[Voice] | None = None
    image_files: list[Path] | None = None  # Images for visual context
    summaries: list[tuple[Path, str]] | None = None  # Pre-generated summaries


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


def _save_clipboard_image() -> Path | None:
    """Save clipboard image to temp file. Returns path or None if no image."""
    fd, path = tempfile.mkstemp(suffix=".png", prefix="clipboard-")
    os.close(fd)
    temp_path = Path(path)

    # Try PNG first, fall back to TIFF, write to file if found
    script = f'''
set theFile to POSIX file "{temp_path}"
try
    set theData to the clipboard as «class PNGf»
on error
    try
        set theData to the clipboard as «class TIFF»
    on error
        return "none"
    end try
end try
try
    set fileRef to open for access theFile with write permission
    write theData to fileRef
    close access fileRef
    return "ok"
on error
    try
        close access theFile
    end try
    return "error"
end try
'''
    result = subprocess.run(
        ["osascript", "-e", script],
        capture_output=True,
        text=True,
    )

    if result.returncode == 0 and result.stdout.strip() == "ok":
        return temp_path

    temp_path.unlink(missing_ok=True)
    return None


def _read_clipboard() -> ClipboardContent | None:
    """Read clipboard content - text, image, or both."""
    text = None
    image_path = None

    # Check for text
    result = subprocess.run(["pbpaste"], capture_output=True, text=True)
    if result.returncode == 0 and result.stdout.strip():
        text = result.stdout

    # Check for image
    image_path = _save_clipboard_image()

    if text or image_path:
        return ClipboardContent(text=text, image_path=image_path)
    return None


def _get_builtin_task(name: str) -> Path | None:
    """Return path to bundled template if it exists."""
    builtin = _TEMPLATES_DIR / f"{name}.md"
    return builtin if builtin.exists() else None


def list_builtin_tasks() -> list[str]:
    """Return names of all builtin tasks."""
    if not _TEMPLATES_DIR.exists():
        return []
    return sorted(p.stem for p in _TEMPLATES_DIR.glob("*.md"))


# Files in .lf/ that aren't tasks (prompts, docs, etc)
_LF_NON_TASK_FILES = {
    "config.yaml",
    "config.yml",
    "COMMIT_MESSAGE.md",
    "CHECKPOINT_MESSAGE.md",
}


def list_user_tasks(repo_root: Path) -> list[str]:
    """Return names of user-defined tasks in the repo."""
    tasks = set()

    # .claude/commands/*.md
    claude_dir = repo_root / ".claude" / "commands"
    if claude_dir.exists():
        for p in claude_dir.glob("*.md"):
            tasks.add(p.stem)

    # .lf/*.md
    lf_dir = repo_root / ".lf"
    if lf_dir.exists():
        for p in lf_dir.glob("*.md"):
            if p.name in _LF_NON_TASK_FILES:
                continue
            # Skip uppercase files (likely docs/prompts, not tasks)
            if p.stem.isupper():
                continue
            tasks.add(p.stem)

    return sorted(tasks)


def list_all_tasks(repo_root: Path | None, config=None) -> tuple[list[str], list[str], list[tuple[str, str]]]:
    """Return (user_tasks, builtin_only_tasks, external_skills) for discoverability.

    User tasks include any that override builtins.
    Builtin-only tasks are builtins not overridden by user tasks.
    External skills are (prefixed_name, source_name) tuples from skill sources.
    """
    builtins = set(list_builtin_tasks())
    if repo_root:
        user = set(list_user_tasks(repo_root))
    else:
        user = set()

    builtin_only = builtins - user

    # Discover external skills
    skill_source_configs = None
    if config and config.skill_sources:
        skill_source_configs = [
            {"name": s.name, "prefix": s.prefix, "path": s.path}
            for s in config.skill_sources
        ]
    sources = discover_skill_sources(skill_source_configs, repo_root)
    external_skills = list_all_skills(sources)

    return sorted(user), sorted(builtin_only), external_skills


def gather_task(repo_root: Path | None, name: str, config=None) -> TaskFile | None:
    """Gather and parse task file with frontmatter.

    Search order:
    1. External skills (prefix:name format, e.g., sp:brainstorm)
    2. .claude/commands/{name}.md (if repo_root provided)
    3. .lf/{name}.md (if repo_root provided)
    4. templates/commands/{name}.md (builtin fallback)

    Returns TaskFile with parsed config, or None if not found.
    """
    # Check for external skill (prefix:name format)
    if ":" in name:
        skill_source_configs = None
        if config and config.skill_sources:
            skill_source_configs = [
                {"name": s.name, "prefix": s.prefix, "path": s.path}
                for s in config.skill_sources
            ]
        sources = discover_skill_sources(skill_source_configs, repo_root)
        skill = find_skill(name, sources)
        if skill:
            content = load_skill_prompt(skill)
            return parse_task_file(name, content)

    if repo_root:
        # Check .claude/commands first (portable format)
        claude_dir = repo_root / ".claude" / "commands"
        content = _read_file_if_named(claude_dir, f"{name}.md")
        if content:
            return parse_task_file(name, content)

        # Fall back to .lf directory
        lf_dir = repo_root / ".lf"
        content = _read_file_if_named(lf_dir, f"{name}.md")
        if content:
            return parse_task_file(name, content)

    # Fall back to builtin templates
    builtin_path = _get_builtin_task(name)
    if builtin_path:
        content = builtin_path.read_text()
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


def _trigger_background_refresh(repo_root: Path) -> None:
    """Spawn background process to refresh stale summaries.

    Uses a lock file to prevent concurrent refresh attempts.
    Logs output to .lf/summaries/refresh.log for debugging.
    """
    summaries_dir = repo_root / ".lf" / "summaries"
    summaries_dir.mkdir(parents=True, exist_ok=True)
    lock_file = summaries_dir / ".refresh.lock"
    log_file = summaries_dir / "refresh.log"

    # Check if refresh already in progress
    if lock_file.exists():
        try:
            pid = int(lock_file.read_text().strip())
            # Check if process still running
            os.kill(pid, 0)
            return  # Refresh already in progress
        except (ValueError, OSError, ProcessLookupError):
            # Stale lock or process dead, remove it
            lock_file.unlink(missing_ok=True)

    # Fork and write lock, logging output for debugging
    with open(log_file, "w") as log:
        process = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfops", "summarize", "--all"],
            cwd=repo_root,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    lock_file.write_text(str(process.pid))


def gather_summaries(repo_root: Path, config) -> list[tuple[Path, str]]:
    """Load all configured summaries for context inclusion.

    Returns cached summaries immediately. If any are stale or missing,
    triggers background refresh for next run.
    """
    if not config or not config.summaries:
        return []

    results = []
    needs_refresh = False

    for summary_config in config.summaries:
        token_budget = summary_config.tokens or config.summary_tokens
        summary = load_summary(Path(summary_config.path), repo_root, token_budget)
        if summary:
            results.append((Path(summary_config.path), summary.content))
            if is_stale(summary, repo_root):
                needs_refresh = True
        else:
            needs_refresh = True

    if needs_refresh:
        _trigger_background_refresh(repo_root)

    return results


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
    include_summaries: bool = True,
    config=None,
    with_prompts: Optional[list[str]] = None,
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
        task_file = gather_task(repo_root, task, config)
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
            # Append additional prompts if provided
            if with_prompts:
                for prompt_name in with_prompts:
                    prompt_file = gather_task(repo_root, prompt_name, config)
                    if prompt_file:
                        task_content = task_content.rstrip() + "\n\n---\n\n" + prompt_file.content
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
    gather_result = gather_files(all_file_paths, repo_root, context_exclude)

    clipboard = _read_clipboard() if paste else None

    # Load voices if specified
    loaded_voices = [load_voice(name, repo_root) for name in voices] if voices else None

    # Load configured summaries
    summaries = gather_summaries(repo_root, config) if include_summaries else None

    return PromptComponents(
        run_mode=run_mode,
        docs=docs,
        diff=diff,
        diff_files=gather_result.text_files,
        task=task_result,
        repo_root=repo_root,
        clipboard=clipboard,
        loopflow_doc=loopflow_doc,
        voices=loaded_voices,
        image_files=gather_result.image_files or None,
        summaries=summaries,
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

    if components.summaries:
        summary_parts = []
        for summary_path, content in components.summaries:
            summary_parts.append(f"<lf:summary path=\"{summary_path}\">\n{content}\n</lf:summary>")
        summaries_body = "\n\n".join(summary_parts)
        parts.append(
            "Pre-generated codebase summaries.\n\n"
            f"<lf:summaries>\n{summaries_body}\n</lf:summaries>"
        )

    if components.diff:
        parts.append(f"Changes on this branch (diff against main).\n\n<lf:diff>\n{components.diff}\n</lf:diff>")

    # diff_files now contains merged diff + context files (deduplicated at load time)
    if components.diff_files:
        parts.append(format_files(components.diff_files, components.repo_root))

    # Handle clipboard content (text and/or image)
    if components.clipboard and components.clipboard.text:
        parts.append(f"Content from clipboard.\n\n<lf:clipboard>\n{components.clipboard.text}\n</lf:clipboard>")

    # Merge clipboard image with other image files
    all_images = list(components.image_files) if components.image_files else []
    if components.clipboard and components.clipboard.image_path:
        all_images.insert(0, components.clipboard.image_path)

    if all_images:
        parts.append(format_image_references(all_images, components.repo_root))

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
