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


def generate_commit_message(repo_root: Path) -> CommitMessage:
    """Generate commit message for staged changes."""
    parts = []

    # Include repo docs for context (STYLE, etc.)
    root_docs = gather_docs(repo_root, repo_root)
    if root_docs:
        doc_parts = []
        for doc_path, content in root_docs:
            name = doc_path.stem
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(f"<lf:docs>\n{docs_body}\n</lf:docs>")

    # Staged diff is the input
    diff = _get_staged_diff(repo_root)
    if diff:
        parts.append(f"<lf:diff>\n{diff}\n</lf:diff>")

    # The task instructions
    task_prompt = get_builtin_prompt("commit_message")
    parts.append(f"<lf:task>\n{task_prompt}\n</lf:task>")

    prompt = "\n\n".join(parts)

    agent = Agent(
        "anthropic:claude-sonnet-4-20250514",
        output_type=CommitMessage,
    )
    result = agent.run_sync(prompt)
    return result.output


def generate_pr_message(repo_root: Path) -> CommitMessage:
    """Generate PR title and body from the branch diff."""
    parts = []

    # Include repo docs for context (STYLE, VOICE, etc.)
    root_docs = gather_docs(repo_root, repo_root)
    if root_docs:
        doc_parts = []
        for doc_path, content in root_docs:
            name = doc_path.stem
            doc_parts.append(f"<lf:{name}>\n{content}\n</lf:{name}>")
        docs_body = "\n\n".join(doc_parts)
        parts.append(f"<lf:docs>\n{docs_body}\n</lf:docs>")

    # The diff is the main input
    diff = gather_diff(repo_root)
    if diff:
        parts.append(f"<lf:diff>\n{diff}\n</lf:diff>")

    # The task instructions
    task_prompt = get_builtin_prompt("pr_message")
    parts.append(f"<lf:task>\n{task_prompt}\n</lf:task>")

    prompt = "\n\n".join(parts)

    agent = Agent(
        "anthropic:claude-sonnet-4-20250514",
        output_type=CommitMessage,
    )
    result = agent.run_sync(prompt)
    return result.output
