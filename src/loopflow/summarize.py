"""Codebase summarization for LLM context."""

import hashlib
import json
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from pydantic_ai import Agent

from loopflow.builtins import get_builtin_prompt
from loopflow.files import _compile_exclude_patterns, _is_ignored, is_binary


@dataclass
class Summary:
    """A generated codebase summary."""

    path: Path
    content: str
    token_budget: int
    source_hash: str
    created_at: datetime
    model: str


@dataclass
class SummaryMetadata:
    """Metadata for a single summary."""

    source_hash: str
    token_budget: int
    created_at: str
    model: str


def _path_to_filename(path: Path, token_budget: int) -> str:
    """Convert path and token budget to summary filename."""
    base = "root" if path == Path(".") else str(path).replace("/", "-").replace("\\", "-")
    return f"{base}-{token_budget}.md"


def _summaries_dir(repo_root: Path) -> Path:
    return repo_root / ".lf" / "summaries"


def _metadata_path(repo_root: Path) -> Path:
    return _summaries_dir(repo_root) / "_metadata.json"


def _load_metadata(repo_root: Path) -> dict[str, SummaryMetadata]:
    """Load metadata for all summaries."""
    path = _metadata_path(repo_root)
    if not path.exists():
        return {}

    data = json.loads(path.read_text())
    result = {}
    for key, val in data.items():
        result[key] = SummaryMetadata(
            source_hash=val["source_hash"],
            token_budget=val["token_budget"],
            created_at=val["created_at"],
            model=val["model"],
        )
    return result


def _save_metadata(repo_root: Path, metadata: dict[str, SummaryMetadata]) -> None:
    """Save metadata for all summaries."""
    path = _metadata_path(repo_root)
    path.parent.mkdir(parents=True, exist_ok=True)

    data = {}
    for key, val in metadata.items():
        data[key] = {
            "source_hash": val.source_hash,
            "token_budget": val.token_budget,
            "created_at": val.created_at,
            "model": val.model,
        }
    path.write_text(json.dumps(data, indent=2) + "\n")


def _ensure_gitignored(repo_root: Path) -> None:
    """Ensure .lf/summaries/ is in .gitignore."""
    gitignore = repo_root / ".gitignore"
    pattern = ".lf/summaries/"

    if gitignore.exists():
        content = gitignore.read_text()
        if pattern in content:
            return
        # Append with newline if file doesn't end with one
        if content and not content.endswith("\n"):
            content += "\n"
        content += pattern + "\n"
        gitignore.write_text(content)
    else:
        gitignore.write_text(pattern + "\n")


def load_summary(path: Path, repo_root: Path, token_budget: int) -> Summary | None:
    """Load cached summary from .lf/summaries/.

    Returns None if no summary exists for this path and token budget.
    """
    filename = _path_to_filename(path, token_budget)
    summary_path = _summaries_dir(repo_root) / filename

    if not summary_path.exists():
        return None

    metadata = _load_metadata(repo_root)
    key = f"{path}:{token_budget}"
    if key not in metadata:
        return None

    meta = metadata[key]
    return Summary(
        path=path,
        content=summary_path.read_text(),
        token_budget=meta.token_budget,
        source_hash=meta.source_hash,
        created_at=datetime.fromisoformat(meta.created_at),
        model=meta.model,
    )


def save_summary(summary: Summary, repo_root: Path) -> None:
    """Save summary to .lf/summaries/."""
    _ensure_gitignored(repo_root)

    summaries_dir = _summaries_dir(repo_root)
    summaries_dir.mkdir(parents=True, exist_ok=True)

    filename = _path_to_filename(summary.path, summary.token_budget)
    summary_path = summaries_dir / filename
    summary_path.write_text(summary.content)

    metadata = _load_metadata(repo_root)
    key = f"{summary.path}:{summary.token_budget}"
    metadata[key] = SummaryMetadata(
        source_hash=summary.source_hash,
        token_budget=summary.token_budget,
        created_at=summary.created_at.isoformat(),
        model=summary.model,
    )
    _save_metadata(repo_root, metadata)


def hash_content(content: str) -> str:
    """Hash content for staleness detection."""
    return hashlib.sha256(content.encode()).hexdigest()[:16]


def _get_merge_base(repo_root: Path) -> str | None:
    """Get the merge-base between HEAD and main."""
    result = subprocess.run(
        ["git", "merge-base", "main", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def _hash_directory_git(path: Path, repo_root: Path) -> str | None:
    """Hash directory contents at the merge-base with main.

    Uses the diff base so summaries only refresh when main advances,
    not on local branch modifications (which are already visible via diff_files).
    """
    base = _get_merge_base(repo_root)
    if not base:
        return None

    target = str(path) if path != Path(".") else ""
    cmd = ["git", "ls-tree", "-r", base]
    if target:
        cmd.extend(["--", target])
    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return hash_content(result.stdout)


def compute_source_hash(path: Path, repo_root: Path) -> str:
    """Compute hash for source content under path."""
    full_path = repo_root if path == Path(".") else repo_root / path

    if full_path.is_file():
        return hash_content(full_path.read_text())

    # Try git-based hash first
    git_hash = _hash_directory_git(path, repo_root)
    if git_hash:
        return git_hash

    # Fallback: hash all file paths and mtimes
    parts = []
    for p in sorted(full_path.rglob("*")):
        if p.is_file():
            parts.append(f"{p}:{p.stat().st_mtime}")
    return hash_content("\n".join(parts))


def is_stale(summary: Summary, repo_root: Path) -> bool:
    """Check if source content changed since summary was generated."""
    current_hash = compute_source_hash(summary.path, repo_root)
    return current_hash != summary.source_hash


def _list_files_at_commit(commit: str, path: Path, repo_root: Path) -> list[str]:
    """List files under path at a specific commit."""
    target = str(path) if path != Path(".") else ""
    cmd = ["git", "ls-tree", "-r", "--name-only", commit]
    if target:
        cmd.extend(["--", target])
    result = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True)
    if result.returncode != 0:
        return []
    return [f for f in result.stdout.strip().split("\n") if f]


def _read_file_at_commit(commit: str, filepath: str, repo_root: Path) -> str | None:
    """Read file content at a specific commit."""
    result = subprocess.run(
        ["git", "show", f"{commit}:{filepath}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout


def gather_source_content(path: Path, repo_root: Path, exclude: list[str] | None = None) -> str:
    """Collect file contents under path at merge-base for summarization.

    Reads from merge-base so summary reflects the base codebase state,
    not local branch changes (which are visible via diff_files).
    """
    base = _get_merge_base(repo_root)
    if not base:
        # Fallback to working directory if no merge-base (e.g., on main)
        return _gather_source_content_working_dir(path, repo_root, exclude)

    excluded_paths = _compile_exclude_patterns(exclude or [], repo_root) if exclude else None
    files = _list_files_at_commit(base, path, repo_root)

    parts = []
    for filepath in sorted(files):
        file_path = repo_root / filepath
        if excluded_paths and _is_ignored(file_path, repo_root, excluded_paths):
            continue
        if ".lf/summaries" in filepath:
            continue
        # Skip binary files by extension
        if is_binary(file_path):
            continue

        content = _read_file_at_commit(base, filepath, repo_root)
        if content is None:
            continue

        parts.append(f"# {filepath}\n\n```\n{content}\n```")

    return "\n\n".join(parts)


def _gather_source_content_working_dir(path: Path, repo_root: Path, exclude: list[str] | None = None) -> str:
    """Fallback: collect file contents from working directory."""
    full_path = repo_root if path == Path(".") else repo_root / path
    excluded_paths = _compile_exclude_patterns(exclude or [], repo_root) if exclude else None

    parts = []

    if full_path.is_file():
        if not is_binary(full_path):
            rel = full_path.relative_to(repo_root)
            parts.append(f"# {rel}\n\n```\n{full_path.read_text()}\n```")
        return "\n\n".join(parts)

    for p in sorted(full_path.rglob("*")):
        if not p.is_file():
            continue
        if _is_ignored(p, repo_root, excluded_paths):
            continue
        if is_binary(p):
            continue
        try:
            content = p.read_text()
        except (OSError, UnicodeDecodeError):
            continue

        rel = p.relative_to(repo_root)
        if ".lf/summaries" in str(rel):
            continue
        parts.append(f"# {rel}\n\n```\n{content}\n```")

    return "\n\n".join(parts)


def _load_summarize_prompt(repo_root: Path) -> str:
    """Load summarize prompt, checking for override first."""
    override = repo_root / ".lf" / "SUMMARIZE.md"
    if override.exists():
        return override.read_text()
    return get_builtin_prompt("summarize")


def _get_model_name(model: str) -> str:
    """Convert short model name to pydantic_ai model identifier."""
    model_map = {
        "gemini": "google-gla:gemini-2.0-flash",
        "gemini:flash": "google-gla:gemini-2.0-flash",
        "gemini:pro": "google-gla:gemini-2.5-pro",
        "claude": "anthropic:claude-sonnet-4-20250514",
        "claude:sonnet": "anthropic:claude-sonnet-4-20250514",
        "claude:opus": "anthropic:claude-opus-4-20250514",
    }
    return model_map.get(model, model)


def _estimate_tokens(text: str) -> int:
    """Rough token estimate (~4 chars per token)."""
    return len(text) // 4


def generate_summary(
    path: Path,
    repo_root: Path,
    token_budget: int,
    model: str = "gemini",
    exclude: list[str] | None = None,
) -> Summary:
    """Generate summary via LLM, respecting token budget.

    If source content fits within budget, returns it directly without LLM call.
    """
    source_content = gather_source_content(path, repo_root, exclude)
    source_hash = compute_source_hash(path, repo_root)

    # If source fits in budget, use it directly
    estimated_tokens = _estimate_tokens(source_content)
    if estimated_tokens <= token_budget:
        return Summary(
            path=path,
            content=source_content,
            token_budget=token_budget,
            source_hash=source_hash,
            created_at=datetime.now(),
            model="raw",  # No summarization needed
        )

    prompt_template = _load_summarize_prompt(repo_root)
    prompt = prompt_template.format(token_budget=token_budget, content=source_content)

    model_name = _get_model_name(model)
    agent = Agent(model_name, output_type=str)
    result = agent.run_sync(prompt)

    return Summary(
        path=path,
        content=result.output,
        token_budget=token_budget,
        source_hash=source_hash,
        created_at=datetime.now(),
        model=model,
    )


def refresh_if_stale(
    path: Path,
    repo_root: Path,
    token_budget: int,
    model: str = "gemini",
    exclude: list[str] | None = None,
    force: bool = False,
) -> tuple[Summary, bool]:
    """Load cached summary or regenerate if stale.

    Returns (summary, was_regenerated).
    """
    if not force:
        existing = load_summary(path, repo_root, token_budget)
        if existing and not is_stale(existing, repo_root):
            return existing, False

    summary = generate_summary(path, repo_root, token_budget, model, exclude)
    save_summary(summary, repo_root)
    return summary, True
