"""LLM API integration for structured responses."""

import json
import os
import subprocess
import sys
from pathlib import Path

from pydantic import BaseModel
from pydantic_ai import Agent

from loopflow.config import load_config, parse_model
from loopflow.context import gather_diff, gather_docs
from loopflow.launcher import build_claude_command, build_codex_command
from loopflow.logging import get_model_env
from loopflow.builtins import get_builtin_prompt

# Always use Sonnet for commit/PR messages - fast and cheap (~$0.005/message)
_COMMIT_MODEL = "anthropic:claude-sonnet-4-20250514"


class CommitMessage(BaseModel):
    """A commit/PR message with title and body."""

    title: str
    body: str


class ReleaseNotes(BaseModel):
    """Release notes for a version bump."""

    summary: str
    changes: list[str]


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


def _commit_debug_enabled() -> bool:
    return os.environ.get("LF_COMMIT_DEBUG") == "1"


def _log_api_failure(action: str, error: Exception) -> None:
    key_set = "yes" if os.environ.get("ANTHROPIC_API_KEY") else "no"
    msg = (
        f"[lf] {action} via API failed ({type(error).__name__}): {error}. "
        f"ANTHROPIC_API_KEY set: {key_set}."
    )
    print(msg, file=sys.stderr)


def _log_cli_failure(action: str, error: Exception) -> None:
    msg = f"[lf] {action} via CLI failed ({type(error).__name__}): {error}."
    print(msg, file=sys.stderr)


def _log_success(action: str, path: str) -> None:
    if not _commit_debug_enabled():
        return
    print(f"[lf] {action} via {path} ok", file=sys.stderr)


def _extract_json_payload(text: str) -> dict | None:
    text = text.strip()
    if not text:
        return None
    try:
        payload = json.loads(text)
        if isinstance(payload, dict):
            return payload
    except json.JSONDecodeError:
        pass

    start = text.find("{")
    end = text.rfind("}")
    if start == -1 or end == -1 or start >= end:
        return None
    try:
        payload = json.loads(text[start : end + 1])
    except json.JSONDecodeError:
        return None
    return payload if isinstance(payload, dict) else None


def _parse_cli_message(output: str) -> CommitMessage:
    payload = _extract_json_payload(output)
    if payload and "title" in payload and "body" in payload:
        return CommitMessage(title=payload["title"], body=payload["body"])

    lines = [line.strip() for line in output.splitlines() if line.strip()]
    if not lines:
        raise ValueError("Empty commit message output")
    title = lines[0]
    body = "\n".join(lines[1:]) if len(lines) > 1 else ""
    return CommitMessage(title=title, body=body)


def _generate_message_via_cli(repo_root: Path, prompt: str) -> CommitMessage:
    config = load_config(repo_root)
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    if backend == "codex":
        cmd = build_codex_command(
            auto=True,
            stream=False,
            skip_permissions=True,
            model_variant=model_variant,
            sandbox_root=repo_root.parent,
            workdir=repo_root,
        )
    else:
        cmd = build_claude_command(
            auto=True,
            stream=False,
            skip_permissions=True,
            model_variant=model_variant,
        )

    cmd_with_prompt = cmd + [prompt]
    result = subprocess.run(
        cmd_with_prompt,
        cwd=repo_root,
        text=True,
        capture_output=True,
        env=get_model_env(),
    )
    output = result.stdout.strip() if result.stdout else ""
    if result.returncode != 0 or not output:
        detail = result.stderr.strip() if result.stderr else "CLI failed"
        raise RuntimeError(detail)
    return _parse_cli_message(output)


def _generate_message(repo_root: Path, prompt: str, action: str) -> CommitMessage:
    """Generate a message via API, falling back to CLI on failure."""
    agent = Agent(_COMMIT_MODEL, output_type=CommitMessage)
    try:
        result = agent.run_sync(prompt)
        _log_success(action, "API")
        return result.output
    except Exception as e:
        _log_api_failure(action, e)
        cli_prompt = prompt + "\n\nReturn JSON with keys: title, body. No extra text."
        try:
            message = _generate_message_via_cli(repo_root, cli_prompt)
            _log_success(action, "CLI")
            return message
        except Exception as cli_error:
            _log_cli_failure(action, cli_error)
            raise


def generate_commit_message(repo_root: Path) -> CommitMessage:
    """Generate commit message for staged changes."""
    diff = _get_staged_diff(repo_root)
    task_prompt = _load_prompt(repo_root, "COMMIT_MESSAGE.md", "commit_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "commit message")


def generate_commit_message_from_diff(repo_root: Path, diff: str | None) -> CommitMessage:
    """Generate commit message for a provided diff."""
    task_prompt = _load_prompt(repo_root, "COMMIT_MESSAGE.md", "commit_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "commit message")


def generate_pr_message(repo_root: Path) -> CommitMessage:
    """Generate PR title and body from the branch diff."""
    diff = gather_diff(repo_root)
    task_prompt = _load_prompt(repo_root, "CHECKPOINT_MESSAGE.md", "pr_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "pr message")


def _get_commits_since_tag(repo_root: Path, tag: str) -> str | None:
    """Get commit log since a tag."""
    result = subprocess.run(
        ["git", "log", f"{tag}..HEAD", "--oneline"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return result.stdout


def generate_release_notes(repo_root: Path, old_version: str, new_version: str) -> ReleaseNotes:
    """Generate release notes from commits since last tag via API."""
    commits = _get_commits_since_tag(repo_root, f"v{old_version}")
    task_prompt = _load_prompt(repo_root, "RELEASE_NOTES.md", "release_notes")

    parts = [f"Version bump: {old_version} → {new_version}"]
    if commits:
        parts.append(f"<commits>\n{commits}\n</commits>")
    parts.append(f"<task>\n{task_prompt}\n</task>")
    prompt = "\n\n".join(parts)

    agent = Agent(_COMMIT_MODEL, output_type=ReleaseNotes)
    try:
        result = agent.run_sync(prompt)
        _log_success("release notes", "API")
        return result.output
    except Exception as e:
        _log_api_failure("release notes", e)
        raise
