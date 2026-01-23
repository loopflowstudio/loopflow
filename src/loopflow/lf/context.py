"""Context gathering for LLM sessions."""

import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from enum import Enum
from importlib import resources
from pathlib import Path
from typing import Optional

from pydantic import BaseModel, Field

from loopflow.lf.design import gather_area_docs, gather_design_docs, gather_internal_docs
from loopflow.lf.files import format_files, format_image_references, gather_docs, gather_files
from loopflow.lf.frontmatter import StepFile, parse_step_file
from loopflow.lf.skills import (
    discover_skill_sources,
    find_skill,
    list_all_skills,
    load_skill_prompt,
)
from loopflow.lf.tokens import count_tokens
from loopflow.lf.voices import Voice, load_voice
from loopflow.lfops.summarize import is_stale, load_summary

# Path to bundled builtin templates
_TEMPLATES_DIR = Path(__file__).parent.parent / "templates" / "steps"

# Global step locations to check (in order)
_GLOBAL_STEP_PATHS = [
    Path.home() / ".lf" / "steps",  # Global loopflow steps
    Path.home() / ".claude" / "commands",  # Claude Code compatibility
]


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
    step: tuple[str, str] | None  # (name, content)
    repo_root: Path
    clipboard: ClipboardContent | None = None
    loopflow_doc: str | None = None  # Bundled system documentation
    voices: list[Voice] | None = None
    image_files: list[Path] | None = None  # Images for visual context
    summaries: list[tuple[Path, str]] | None = None  # Pre-generated summaries


@dataclass(frozen=True)
class DroppedComponent:
    """Component dropped to fit token limits."""

    kind: str
    name: str | None
    tokens: int
    reason: str | None = None


class DiffMode(str, Enum):
    """How to include branch changes in context."""

    FILES = "files"  # Full content of changed files (default for steps)
    DIFF = "diff"  # Raw unified diff (for commits)
    NONE = "none"  # Neither


# Default paths always included unless explicitly excluded
_DEFAULT_FILE_PATHS = ["scratch/", "roadmap/", "*.md"]


class FilesetConfig(BaseModel):
    """Configuration for file context.

    Default paths (scratch/, roadmap/, *.md) are always included unless excluded.
    The paths field is additive to defaults.
    """

    paths: list[str] = Field(default_factory=list)  # Additive to defaults
    exclude: list[str] = Field(default_factory=list)  # Removes from defaults + paths
    token_limit: int | None = None  # If set, summarize files exceeding this
    parent_docs: bool = True  # Include docs from parent area paths


class ContextConfig(BaseModel):
    """Specifies what context to include in a prompt."""

    # Branch work
    diff_mode: DiffMode = DiffMode.FILES

    # User files (defaults: scratch/, roadmap/, *.md)
    files: FilesetConfig = Field(default_factory=FilesetConfig)

    # Area path (e.g., "lf/cli" or "concerto/ui")
    # When set with parent_docs=True, includes parent area docs:
    # area="a/b/c" includes a/*.md, a/roadmap/, a/b/*.md, a/b/roadmap/, etc.
    area: str | None = None

    # Bundled LOOPFLOW.md system documentation
    lfdocs: bool = True

    # Extras
    clipboard: bool = False

    @classmethod
    def for_commit(cls) -> "ContextConfig":
        """Minimal context for commit message generation."""
        return cls(
            diff_mode=DiffMode.DIFF,
            files=FilesetConfig(exclude=["scratch/", "roadmap/", "*.md"]),
            lfdocs=False,
        )

    @classmethod
    def for_interactive(
        cls,
        paths: list[str] | None = None,
        exclude: list[str] | None = None,
        lfdocs: bool = True,
        token_limit: int | None = None,
        clipboard: bool = True,
    ) -> "ContextConfig":
        """Context for interactive sessions with explicit files."""
        return cls(
            diff_mode=DiffMode.NONE,
            files=FilesetConfig(
                paths=paths or [],
                exclude=exclude or [],
                token_limit=token_limit,
            ),
            lfdocs=lfdocs,
            clipboard=clipboard,
        )


def format_drop_label(drop: DroppedComponent) -> str:
    """Format a dropped component for display."""
    if drop.kind == "diff_files":
        return "diff_files"
    if drop.kind in ("docs", "summaries"):
        return f"{drop.kind}:{drop.name}"
    if drop.kind == "diff":
        return "diff"
    if drop.kind == "clipboard":
        return "clipboard"
    return drop.kind


def trim_prompt_components(
    components: PromptComponents, max_tokens: int
) -> tuple[PromptComponents, list[DroppedComponent]]:
    """Drop large components until the prompt fits the token budget."""
    dropped: list[DroppedComponent] = []
    total_tokens = _total_component_tokens(components)
    if total_tokens <= max_tokens:
        return components, dropped

    diff_files_tokens = _diff_files_tokens(components)
    if components.diff_files and diff_files_tokens > max_tokens:
        components.diff_files = []
        dropped.append(
            DroppedComponent("diff_files", None, diff_files_tokens, reason="exceeds limit")
        )
        total_tokens = _total_component_tokens(components)
        if total_tokens <= max_tokens:
            return components, dropped

    while total_tokens > max_tokens:
        candidates = _drop_candidates(components)
        if not candidates:
            break

        candidates.sort(key=lambda item: (-item.tokens, item.kind, item.name or ""))
        candidate = candidates[0]
        if candidate.tokens <= 0:
            break

        _apply_drop_candidate(components, candidate)
        dropped.append(
            DroppedComponent(
                candidate.kind,
                candidate.name,
                candidate.tokens,
                reason="greedy",
            )
        )
        total_tokens -= candidate.tokens

    if total_tokens > max_tokens and components.diff_files:
        diff_files_tokens = _diff_files_tokens(components)
        components.diff_files = []
        dropped.append(
            DroppedComponent("diff_files", None, diff_files_tokens, reason="last resort")
        )

    return components, dropped


@dataclass(frozen=True)
class _DropCandidate:
    kind: str
    name: str | None
    tokens: int
    path: Path | None = None


def _drop_candidates(components: PromptComponents) -> list[_DropCandidate]:
    candidates: list[_DropCandidate] = []

    if components.docs:
        for doc_path, content in components.docs:
            candidates.append(
                _DropCandidate("docs", doc_path.name, count_tokens(content), path=doc_path)
            )

    if components.summaries:
        for summary_path, content in components.summaries:
            candidates.append(
                _DropCandidate(
                    "summaries",
                    str(summary_path),
                    count_tokens(content),
                    path=summary_path,
                )
            )

    if components.diff:
        candidates.append(_DropCandidate("diff", "branch diff", count_tokens(components.diff)))

    if components.clipboard and components.clipboard.text:
        candidates.append(
            _DropCandidate(
                "clipboard",
                "pasted text",
                count_tokens(components.clipboard.text),
            )
        )

    return candidates


def _apply_drop_candidate(components: PromptComponents, candidate: _DropCandidate) -> None:
    if candidate.kind == "docs":
        components.docs = [
            (path, content) for path, content in components.docs if path != candidate.path
        ]
    elif candidate.kind == "summaries":
        components.summaries = [
            (path, content)
            for path, content in components.summaries or []
            if path != candidate.path
        ]
    elif candidate.kind == "diff":
        components.diff = None
    elif candidate.kind == "clipboard":
        components.clipboard = None


def _diff_files_tokens(components: PromptComponents) -> int:
    if not components.diff_files:
        return 0
    return sum(count_tokens(content) for _path, content in components.diff_files)


def _total_component_tokens(components: PromptComponents) -> int:
    total = 0
    if components.loopflow_doc:
        total += count_tokens(components.loopflow_doc)
    if components.docs:
        total += sum(count_tokens(content) for _path, content in components.docs)
    if components.diff:
        total += count_tokens(components.diff)
    if components.diff_files:
        total += sum(count_tokens(content) for _path, content in components.diff_files)
    if components.summaries:
        total += sum(count_tokens(content) for _path, content in components.summaries)
    if components.step:
        _name, content = components.step
        total += count_tokens(content)
    if components.clipboard and components.clipboard.text:
        total += count_tokens(components.clipboard.text)
    return total


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


def _get_builtin_step(name: str) -> Path | None:
    """Return path to bundled template if it exists."""
    builtin = _TEMPLATES_DIR / f"{name}.md"
    return builtin if builtin.exists() else None


def list_builtin_steps() -> list[str]:
    """Return names of all builtin steps."""
    if not _TEMPLATES_DIR.exists():
        return []
    return sorted(p.stem for p in _TEMPLATES_DIR.glob("*.md"))


# Files in .lf/ that aren't steps (prompts, docs, etc)
_LF_NON_STEP_FILES = {
    "config.yaml",
    "config.yml",
    "COMMIT_MESSAGE.md",
    "CHECKPOINT_MESSAGE.md",
}


def list_user_steps(repo_root: Path) -> list[str]:
    """Return names of user-defined steps in the repo."""
    steps = set()

    # .lf/steps/*.md (preferred)
    steps_dir = repo_root / ".lf" / "steps"
    if steps_dir.exists():
        for p in steps_dir.glob("*.md"):
            steps.add(p.stem)

    # .claude/commands/*.md (Claude Code compatible)
    claude_dir = repo_root / ".claude" / "commands"
    if claude_dir.exists():
        for p in claude_dir.glob("*.md"):
            steps.add(p.stem)

    # .lf/*.md (legacy)
    lf_dir = repo_root / ".lf"
    if lf_dir.exists():
        for p in lf_dir.glob("*.md"):
            if p.name in _LF_NON_STEP_FILES:
                continue
            # Skip uppercase files (likely docs/prompts, not steps)
            if p.stem.isupper():
                continue
            steps.add(p.stem)

    return sorted(steps)


def list_global_steps() -> list[str]:
    """Return names of globally-installed steps (e.g., ~/.claude/commands/)."""
    steps = set()
    for global_dir in _GLOBAL_STEP_PATHS:
        if global_dir.exists():
            for p in global_dir.glob("*.md"):
                steps.add(p.stem)
    return sorted(steps)


def list_all_steps(
    repo_root: Path | None, config=None
) -> tuple[list[str], list[str], list[str], list[tuple[str, str]]]:
    """Return (user_steps, global_steps, builtin_only_steps, external_skills).

    User steps include any that override builtins or globals.
    Global steps are from ~/.claude/commands/ not overridden by repo-local.
    Builtin-only steps are builtins not overridden by user or global steps.
    External skills are (prefixed_name, source_name) tuples from skill sources.
    """
    builtins = set(list_builtin_steps())
    user = set(list_user_steps(repo_root)) if repo_root else set()
    global_steps = set(list_global_steps())

    sources = discover_skill_sources(
        config.skill_sources if config else None,
        repo_root,
        registry_config=config.skill_registry if config else None,
    )
    external_skills = list_all_skills(sources)

    # Collect skill names that are handled by external sources (to exclude from global)
    external_skill_names = set()
    for source in sources:
        if source.kind == "single-file":
            # Single-file skills like rams are named after the file
            external_skill_names.update(source.skills)

    # Global steps not overridden by repo-local or handled by external sources
    global_only = global_steps - user - external_skill_names
    # Builtins not overridden by user or global
    builtin_only = builtins - user - global_steps

    return sorted(user), sorted(global_only), sorted(builtin_only), external_skills


def gather_step(repo_root: Path | None, name: str, config=None) -> StepFile | None:
    """Gather and parse step file with frontmatter.

    Search order:
    1. External skills (prefix:name format, e.g., sp:brainstorm)
    2. .lf/steps/{name}.md (repo)
    3. .claude/commands/{name}.md (repo, Claude Code compatible)
    4. ~/.lf/steps/{name}.md (global)
    5. ~/.claude/commands/{name}.md (global, Claude Code compatible)
    6. templates/steps/{name}.md (builtin)

    Returns StepFile with parsed config, or None if not found.
    """
    if ":" in name:
        sources = discover_skill_sources(
            config.skill_sources if config else None,
            repo_root,
            registry_config=config.skill_registry if config else None,
        )
        skill = find_skill(name, sources)
        if skill:
            content = load_skill_prompt(skill)
            step_file = parse_step_file(name, content)
            step_file.is_external_skill = True
            # External skills default to interactive (if not specified in frontmatter)
            if step_file.config.interactive is None:
                step_file.config.interactive = True
            return step_file

    if repo_root:
        # Check .lf/steps/ first
        lf_steps_dir = repo_root / ".lf" / "steps"
        content = _read_file_if_named(lf_steps_dir, f"{name}.md")
        if content:
            return parse_step_file(name, content)

        # Check .claude/commands (Claude Code compatible)
        claude_dir = repo_root / ".claude" / "commands"
        content = _read_file_if_named(claude_dir, f"{name}.md")
        if content:
            return parse_step_file(name, content)

    # Check global user steps
    for global_dir in _GLOBAL_STEP_PATHS:
        content = _read_file_if_named(global_dir, f"{name}.md")
        if content:
            return parse_step_file(name, content)

    # Fall back to builtin templates
    builtin_path = _get_builtin_step(name)
    if builtin_path:
        content = builtin_path.read_text()
        return parse_step_file(name, content)

    return None


def gather_diff(
    repo_root: Path,
    exclude: Optional[list[str]] = None,
    base_ref: str | None = None,
) -> str | None:
    """Get diff against base branch. Returns None if on main or no diff."""
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

    # Get diff against base, excluding specified patterns
    if base_ref is None:
        from loopflow.lf.git import get_default_base_ref

        base_ref = get_default_base_ref(repo_root)
    cmd = ["git", "diff", f"{base_ref}...HEAD"]
    if exclude:
        cmd.append("--")
        cmd.extend(f":(exclude){pattern}" for pattern in exclude)

    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        encoding="utf-8",
        errors="replace",  # Handle binary content in diffs
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None

    return result.stdout


def gather_diff_files(repo_root: Path, base_ref: str | None = None) -> list[str]:
    """Return file paths touched by this branch vs base branch.

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

    if base_ref is None:
        from loopflow.lf.git import get_default_base_ref

        base_ref = get_default_base_ref(repo_root)

    result = subprocess.run(
        ["git", "diff", "--name-only", f"{base_ref}...HEAD"],
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

    Uses a lock file in ~/.lf to prevent concurrent refresh attempts.
    Logs output to ~/.lf/refresh.log for debugging.
    """
    lf_dir = Path.home() / ".lf"
    lf_dir.mkdir(parents=True, exist_ok=True)
    lock_file = lf_dir / ".refresh.lock"
    log_file = lf_dir / "refresh.log"

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
    step: Optional[str] = None,
    inline: Optional[str] = None,
    step_args: Optional[list[str]] = None,
    run_mode: Optional[str] = None,
    voices: Optional[list[str]] = None,
    context_config: Optional[ContextConfig] = None,
    config=None,
) -> PromptComponents:
    """Gather all prompt components without assembling them."""
    if context_config is None:
        context_config = ContextConfig()

    exclude = context_config.files.exclude or []
    docs = gather_docs(repo_root, repo_root, exclude)

    # Load bundled LOOPFLOW.md (system documentation)
    loopflow_doc = _load_loopflow_doc() if context_config.lfdocs else None

    # Insert design docs and internal docs before repo docs
    # Order: scratch/ (ephemeral), roadmap/ (persistent internal), area docs, repo root .md files
    design_docs = gather_design_docs(repo_root)
    internal_docs = gather_internal_docs(repo_root)

    # Gather area-specific docs if area is set and parent_docs is enabled
    area_docs = []
    if context_config.area and context_config.files.parent_docs:
        area_docs = gather_area_docs(repo_root, context_config.area)

    prefix_docs = design_docs + internal_docs + area_docs
    if prefix_docs:
        docs[0:0] = prefix_docs

    # Gather diff based on mode
    diff = None
    if context_config.diff_mode == DiffMode.DIFF:
        diff = gather_diff(repo_root, exclude)

    step_result = None
    if inline:
        step_result = ("inline", inline)
    elif step:
        step_file = gather_step(repo_root, step, config)
        if step_file:
            step_content = step_file.content
            # Process step_args if provided
            if step_args:
                plain_args = []
                for arg in step_args:
                    if "=" in arg:
                        # Template substitution: {{key}} -> value
                        key, value = arg.split("=", 1)
                        step_content = step_content.replace(f"{{{{{key}}}}}", value)
                    else:
                        plain_args.append(arg)
                # Append plain args to step content
                if plain_args:
                    step_content = step_content.rstrip() + "\n\n" + " ".join(plain_args)
            step_result = (step, step_content)
        else:
            step_result = (step, f"No step file found for '{step}'.")

    context_exclude = list(exclude) if exclude else []

    # Gather file paths based on diff_mode
    diff_file_paths = []
    if context_config.diff_mode == DiffMode.FILES:
        diff_file_paths = gather_diff_files(repo_root)

    # Add explicit paths (additive to diff files)
    explicit_paths = context_config.files.paths or []
    diff_set = set(diff_file_paths)
    all_file_paths = diff_file_paths + [p for p in explicit_paths if p not in diff_set]
    gather_result = gather_files(all_file_paths, repo_root, context_exclude)

    clipboard = _read_clipboard() if context_config.clipboard else None

    # Load voices if specified
    loaded_voices = None
    if voices:
        loaded_voices = [
            load_voice(repo_root, name) for name in voices if load_voice(repo_root, name)
        ]

    # Load configured summaries (always include if config has them)
    summaries = gather_summaries(repo_root, config)

    return PromptComponents(
        run_mode=run_mode,
        docs=docs,
        diff=diff,
        diff_files=gather_result.text_files,
        step=step_result,
        repo_root=repo_root,
        clipboard=clipboard,
        loopflow_doc=loopflow_doc,
        voices=loaded_voices,
        image_files=gather_result.image_files or None,
        summaries=summaries if summaries else None,
    )


def format_prompt(components: PromptComponents) -> str:
    """Format prompt components into the final prompt string."""
    parts = []

    if components.run_mode == "auto":
        parts.append(
            "Run mode is auto (headless). Proceed without pausing for questions. "
            "If you need clarification, make the best assumption you can and append "
            "any open questions to `scratch/questions.md`."
        )

    if components.loopflow_doc:
        parts.append(f"<lf:loopflow>\n{components.loopflow_doc}\n</lf:loopflow>")

    if components.step:
        name, content = components.step
        if name == "inline":
            step_tag = f"<lf:step>\n{content}\n</lf:step>"
        else:
            step_tag = f"<lf:step:{name}>\n{content}\n</lf:step:{name}>"

        # Voices go between "The step." header and the actual step content
        if components.voices:
            if len(components.voices) == 1:
                voice = components.voices[0]
                voice_section = (
                    f"<lf:voice:{voice.name}>\n{voice.content}\n</lf:voice:{voice.name}>"
                )
            else:
                voice_parts = [
                    f"<lf:voice:{voice.name}>\n{voice.content}\n</lf:voice:{voice.name}>"
                    for voice in components.voices
                ]
                voice_section = f"<lf:voices>\n{chr(10).join(voice_parts)}\n</lf:voices>"
            parts.append(f"The step.\n\n{voice_section}\n\n{step_tag}")
        else:
            parts.append(f"The step.\n\n{step_tag}")

    if components.docs:
        doc_parts = []
        for doc_path, content in components.docs:
            name = doc_path.stem
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(
            "Repository documentation. Follow STYLE carefully. "
            "May include design artifacts (scratch/) and internal docs (roadmap/).\n\n"
            f"<lf:docs>\n{docs_body}\n</lf:docs>"
        )

    if components.summaries:
        summary_parts = []
        for summary_path, content in components.summaries:
            summary_parts.append(f'<lf:summary path="{summary_path}">\n{content}\n</lf:summary>')
        summaries_body = "\n\n".join(summary_parts)
        parts.append(
            "Pre-generated codebase summaries.\n\n"
            f"<lf:summaries>\n{summaries_body}\n</lf:summaries>"
        )

    if components.diff:
        parts.append(
            f"Changes on this branch (diff against main).\n\n"
            f"<lf:diff>\n{components.diff}\n</lf:diff>"
        )

    # diff_files now contains merged diff + context files (deduplicated at load time)
    if components.diff_files:
        parts.append(format_files(components.diff_files, components.repo_root))

    # Handle clipboard content (text and/or image)
    if components.clipboard and components.clipboard.text:
        parts.append(
            f"Content from clipboard.\n\n"
            f"<lf:clipboard>\n{components.clipboard.text}\n</lf:clipboard>"
        )

    # Merge clipboard image with other image files
    all_images = list(components.image_files) if components.image_files else []
    if components.clipboard and components.clipboard.image_path:
        all_images.insert(0, components.clipboard.image_path)

    if all_images:
        parts.append(format_image_references(all_images, components.repo_root))

    return "\n\n".join(parts)


def build_prompt(
    repo_root: Path,
    step: Optional[str] = None,
    inline: Optional[str] = None,
    run_mode: Optional[str] = None,
    voices: Optional[list[str]] = None,
    context_config: Optional[ContextConfig] = None,
) -> str:
    """Build the full prompt for an LLM session."""
    components = gather_prompt_components(
        repo_root,
        step,
        inline,
        run_mode=run_mode,
        voices=voices,
        context_config=context_config,
    )
    return format_prompt(components)
