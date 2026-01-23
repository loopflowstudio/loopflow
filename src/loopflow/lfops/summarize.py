"""Codebase summarization for LLM context."""

import hashlib
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from loopflow.lf.tokens import count_tokens


# Patterns for lfdocs content (excluded from summaries when lfdocs is on)
LFDOCS_EXCLUDE_PATTERNS = [
    ".docs/**",
    ".design/**",
    "*.md",  # Root .md files
]


def build_exclude_patterns(config) -> list[str] | None:
    """Build exclude patterns list, including lfdocs patterns if lfdocs is enabled."""
    if not config:
        return None

    patterns = list(config.exclude) if config.exclude else []

    # When lfdocs is on, exclude those paths from summaries (they're already in context)
    if config.lfdocs:
        patterns.extend(LFDOCS_EXCLUDE_PATTERNS)

    return patterns if patterns else None


@dataclass
class Summary:
    """A generated codebase summary."""

    path: Path
    content: str
    token_budget: int
    source_hash: str
    created_at: datetime
    model: str


def load_summary(path: Path, repo_root: Path, token_budget: int) -> Summary | None:
    """Load cached summary from database.

    Returns None if no summary exists for this path and token budget.
    """
    from loopflow.lfd.db import load_summary_db

    data = load_summary_db(str(repo_root), str(path), token_budget)
    if not data:
        return None

    return Summary(
        path=path,
        content=data["content"],
        token_budget=token_budget,
        source_hash=data["source_hash"],
        created_at=datetime.fromisoformat(data["created_at"]),
        model=data["model"],
    )


def save_summary(summary: Summary, repo_root: Path) -> None:
    """Save summary to database."""
    from loopflow.lfd.db import save_summary_db

    save_summary_db(
        repo=str(repo_root),
        path=str(summary.path),
        token_budget=summary.token_budget,
        source_hash=summary.source_hash,
        content=summary.content,
        model=summary.model,
    )


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
    from loopflow.lf.files import _compile_exclude_patterns, _is_ignored, is_binary

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


def _gather_source_content_working_dir(
    path: Path, repo_root: Path, exclude: list[str] | None = None
) -> str:
    """Fallback: collect file contents from working directory."""
    from loopflow.lf.files import _compile_exclude_patterns, _is_ignored, is_binary

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


# Threshold for recursive summarization (~100k tokens = ~400k chars)
RECURSIVE_THRESHOLD = 400000


def _load_summarize_prompt(repo_root: Path) -> str:
    """Load summarize prompt, checking for override first."""
    from loopflow.lf.builtins import get_builtin_prompt

    override = repo_root / ".lf" / "SUMMARIZE.md"
    if override.exists():
        return override.read_text()
    return get_builtin_prompt("summarize")


def _list_subdirectories(path: Path, repo_root: Path) -> list[Path]:
    """List top-level subdirectories under path at merge-base.

    Returns relative paths from repo_root.
    """
    base = _get_merge_base(repo_root)
    full_path = repo_root if path == Path(".") else repo_root / path

    if base:
        # Use git to list directories at merge-base
        target = str(path) if path != Path(".") else ""
        cmd = ["git", "ls-tree", "--name-only", base]
        if target:
            cmd.extend(["--", target])
        result = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            entries = result.stdout.strip().split("\n")
            subdirs = []
            for entry in entries:
                # Check if it's a directory in the tree
                check = subprocess.run(
                    ["git", "cat-file", "-t", f"{base}:{entry}"],
                    cwd=repo_root,
                    capture_output=True,
                    text=True,
                )
                if check.returncode == 0 and check.stdout.strip() == "tree":
                    subdirs.append(Path(entry))
            return sorted(subdirs)

    # Fallback: use working directory
    if not full_path.is_dir():
        return []

    subdirs = []
    for p in sorted(full_path.iterdir()):
        if p.is_dir() and not p.name.startswith("."):
            subdirs.append(p.relative_to(repo_root))
    return subdirs


def _run_summarize_cli(prompt: str, model: str, repo_root: Path) -> str:
    """Run CLI agent to generate summary."""
    from loopflow.lf.config import parse_model
    from loopflow.lf.launcher import build_model_command
    from loopflow.lf.logging import get_model_env

    backend, model_variant = parse_model(model)
    cmd = build_model_command(
        model=backend,
        auto=True,
        stream=False,
        skip_permissions=True,
        model_variant=model_variant,
        sandbox_root=repo_root.parent,
        workdir=repo_root,
    )

    result = subprocess.run(
        cmd + [prompt],
        cwd=repo_root,
        text=True,
        capture_output=True,
        env=get_model_env(),
    )

    if result.returncode != 0:
        detail = result.stderr.strip() if result.stderr else "CLI failed"
        raise RuntimeError(f"Summary generation failed: {detail}")

    return result.stdout.strip()


def generate_summary(
    path: Path,
    repo_root: Path,
    token_budget: int,
    model: str = "gemini",
    exclude: list[str] | None = None,
) -> Summary:
    """Generate summary via LLM, respecting token budget.

    If source content fits within budget, returns it directly without LLM call.
    For large content exceeding RECURSIVE_THRESHOLD, recursively summarizes subdirectories.
    """
    source_content = gather_source_content(path, repo_root, exclude)
    source_hash = compute_source_hash(path, repo_root)
    content_tokens = count_tokens(source_content)

    # If source fits in budget, use it directly
    if content_tokens <= token_budget:
        return Summary(
            path=path,
            content=source_content,
            token_budget=token_budget,
            source_hash=source_hash,
            created_at=datetime.now(),
            model="raw",  # No summarization needed
        )

    # If content exceeds recursive threshold, split into subdirs
    if len(source_content) > RECURSIVE_THRESHOLD:
        subdirs = _list_subdirectories(path, repo_root)
        if len(subdirs) > 1:
            # Measure each subdir's content size
            subdir_data = []
            for subdir in subdirs:
                subdir_content = gather_source_content(subdir, repo_root, exclude)
                subdir_tokens = count_tokens(subdir_content)
                subdir_data.append((subdir, subdir_tokens, subdir_content))

            # Bin-pack subdirs into groups that fit under model context
            # Model context limit for summarization (leave room for prompt overhead)
            MODEL_CONTEXT = 90000  # ~90k tokens safe for summarization
            total_tokens = sum(t for _, t, _ in subdir_data)

            # Sort by size descending for first-fit-decreasing bin packing
            subdir_data.sort(key=lambda x: x[1], reverse=True)

            groups: list[list[tuple]] = []  # Each group is [(subdir, tokens, content), ...]

            for item in subdir_data:
                subdir, tokens, content = item
                placed = False

                # If single subdir exceeds model context, it needs recursive split
                if tokens > MODEL_CONTEXT:
                    # Recurse on this large subdir
                    groups.append([item])
                    placed = True
                else:
                    # Try to fit in existing group
                    for group in groups:
                        group_tokens = sum(t for _, t, _ in group)
                        if group_tokens + tokens <= MODEL_CONTEXT:
                            group.append(item)
                            placed = True
                            break

                if not placed:
                    groups.append([item])

            # Now summarize each group with proportional budget
            sub_summaries = []

            for group in groups:
                group_tokens = sum(t for _, t, _ in group)
                # Proportional budget based on group's share of total
                group_budget = max((group_tokens * token_budget) // total_tokens, 1000)

                if len(group) == 1 and group[0][1] > MODEL_CONTEXT:
                    # Single large subdir - recurse
                    subdir, _, _ = group[0]
                    sub_summary = generate_summary(subdir, repo_root, group_budget, model, exclude)
                    sub_summaries.append(f"## {subdir}\n\n{sub_summary.content}")
                else:
                    # Group of subdirs - combine and summarize
                    group_content = "\n\n".join(f"## {s}\n\n{c}" for s, _, c in group)

                    if group_tokens <= group_budget:
                        # Small enough to keep raw
                        sub_summaries.append(group_content)
                    else:
                        # Summarize the group
                        prompt_template = _load_summarize_prompt(repo_root)
                        prompt = prompt_template.format(
                            token_budget=group_budget, content=group_content
                        )
                        group_summary = _run_summarize_cli(prompt, model, repo_root)
                        group_names = ", ".join(str(s) for s, _, _ in group)
                        sub_summaries.append(f"## {group_names}\n\n{group_summary}")

            # Concatenate all group summaries
            combined = "\n\n".join(sub_summaries)
            combined_tokens = count_tokens(combined)

            # If combined fits in budget, use it directly
            if combined_tokens <= token_budget:
                return Summary(
                    path=path,
                    content=combined,
                    token_budget=token_budget,
                    source_hash=source_hash,
                    created_at=datetime.now(),
                    model="recursive",
                )

            # Otherwise summarize the combined content
            prompt_template = _load_summarize_prompt(repo_root)
            prompt = prompt_template.format(token_budget=token_budget, content=combined)
            summary_content = _run_summarize_cli(prompt, model, repo_root)

            return Summary(
                path=path,
                content=summary_content,
                token_budget=token_budget,
                source_hash=source_hash,
                created_at=datetime.now(),
                model=f"recursive+{model}",
            )

    # Standard summarization for content that fits in model context
    prompt_template = _load_summarize_prompt(repo_root)
    prompt = prompt_template.format(token_budget=token_budget, content=source_content)
    summary_content = _run_summarize_cli(prompt, model, repo_root)

    return Summary(
        path=path,
        content=summary_content,
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


def register_commands(app) -> None:
    """Register summarize command on the app."""
    import typer

    from loopflow.lf.config import load_config
    from loopflow.lf.context import find_worktree_root

    @app.command()
    def summarize(
        path: str = typer.Argument(".", help="Path to summarize (relative to repo root)"),
        tokens: int = typer.Option(10000, "-t", "--tokens", help="Token budget"),
        model: str = typer.Option("gemini", "-m", "--model", help="Model to use"),
        force: bool = typer.Option(False, "-f", "--force", help="Regenerate even if cached"),
        all_configured: bool = typer.Option(
            False, "-a", "--all", help="Regenerate all configured summaries"
        ),
    ) -> None:
        """Generate a codebase summary."""
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        config = load_config(repo_root)

        if all_configured:
            if not config or not config.summaries:
                typer.echo("No summaries configured in .lf/config.yaml")
                raise typer.Exit(0)

            lock_file = Path.home() / ".lf" / ".refresh.lock"
            try:
                for summary_config in config.summaries:
                    summary_path = Path(summary_config.path)
                    token_budget = summary_config.tokens or config.summary_tokens
                    existing = load_summary(summary_path, repo_root, token_budget)

                    if existing and not force and not is_stale(existing, repo_root):
                        typer.echo(f"  {summary_config.path}: up to date")
                        continue

                    typer.echo(f"  {summary_config.path}: regenerating...")
                    try:
                        summary, _ = refresh_if_stale(
                            summary_path,
                            repo_root,
                            token_budget,
                            summary_config.model,
                            build_exclude_patterns(config),
                            force=force,
                        )
                        typer.echo(f"  {summary_config.path}: done ({len(summary.content)} chars)")
                    except Exception as e:
                        typer.echo(f"  {summary_config.path}: error - {e}", err=True)
            finally:
                lock_file.unlink(missing_ok=True)
            return

        summary_path = Path(path)
        existing = load_summary(summary_path, repo_root, tokens)

        if existing and not force:
            if is_stale(existing, repo_root):
                typer.echo("Summary stale, regenerating...")
            else:
                typer.echo("Summary up to date (use -f to force regenerate)")
                typer.echo(f"  Path: {path}")
                typer.echo(f"  Tokens: {existing.token_budget}")
                typer.echo(f"  Model: {existing.model}")
                typer.echo(f"  Created: {existing.created_at.isoformat()}")
                raise typer.Exit(0)
        else:
            typer.echo(f"Generating summary for {path}...")

        try:
            summary, regenerated = refresh_if_stale(
                summary_path,
                repo_root,
                tokens,
                model,
                build_exclude_patterns(config),
                force=force,
            )
        except Exception as e:
            typer.echo(f"Error generating summary: {e}", err=True)
            raise typer.Exit(1)

        typer.echo("Summary saved to database")
        typer.echo(f"  Path: {summary_path}")
        typer.echo(f"  Token budget: {tokens}")
        typer.echo(f"  Model: {summary.model}")
        typer.echo(f"  Length: {len(summary.content)} chars")
