"""LLM API integration for structured responses."""

from pathlib import Path

from pydantic import BaseModel
from pydantic_ai import Agent

from loopflow.context import gather_diff, gather_docs
from loopflow.builtins import get_builtin_prompt


class CommitMessage(BaseModel):
    """A commit message for a PR."""

    title: str
    body: str


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
