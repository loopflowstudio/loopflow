"""LLM API integration for structured responses."""

import subprocess
from pathlib import Path

from pydantic import BaseModel
from pydantic_ai import Agent

from loopflow.context import gather_diff, gather_docs
from loopflow.builtins import get_builtin_prompt


class CommitMessage(BaseModel):
    """A commit/PR message with title and body."""

    title: str
    body: str


def _get_staged_diff(repo_root: Path) -> str | None:
    """Get diff of staged changes (against HEAD)."""
    result = subprocess.run(
        ["git", "diff", "--cached"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return result.stdout


def _load_prompt(repo_root: Path, filename: str, builtin_name: str) -> str:
    override = repo_root / ".lf" / filename
    if override.exists():
        return override.read_text()
    return get_builtin_prompt(builtin_name)


def _build_message_prompt(repo_root: Path, diff: str | None, task_prompt: str) -> str:
    parts = []

    root_docs = gather_docs(repo_root, repo_root)
    if root_docs:
        doc_parts = []
        for doc_path, content in root_docs:
            name = doc_path.stem
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(f"<lf:docs>\n{docs_body}\n</lf:docs>")

    if diff:
        parts.append(f"<lf:diff>\n{diff}\n</lf:diff>")

    parts.append(f"<lf:task>\n{task_prompt}\n</lf:task>")
    return "\n\n".join(parts)


def generate_commit_message(repo_root: Path) -> CommitMessage:
    """Generate commit message for staged changes."""
    diff = _get_staged_diff(repo_root)
    task_prompt = _load_prompt(repo_root, "COMMIT_MESSAGE.md", "commit_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)

    agent = Agent(
        "anthropic:claude-sonnet-4-20250514",
        output_type=CommitMessage,
    )
    result = agent.run_sync(prompt)
    return result.output


def generate_commit_message_from_diff(repo_root: Path, diff: str | None) -> CommitMessage:
    """Generate commit message for a provided diff."""
    task_prompt = _load_prompt(repo_root, "COMMIT_MESSAGE.md", "commit_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)

    agent = Agent(
        "anthropic:claude-sonnet-4-20250514",
        output_type=CommitMessage,
    )
    result = agent.run_sync(prompt)
    return result.output


def generate_pr_message(repo_root: Path) -> CommitMessage:
    """Generate PR title and body from the branch diff."""
    diff = gather_diff(repo_root)
    task_prompt = _load_prompt(repo_root, "CHECKPOINT_MESSAGE.md", "pr_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)

    agent = Agent(
        "anthropic:claude-sonnet-4-20250514",
        output_type=CommitMessage,
    )
    result = agent.run_sync(prompt)
    return result.output
