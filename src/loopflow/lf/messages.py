"""LLM integration for structured responses via CLI agents."""

import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from loopflow.lf.builtins.prompts import get_builtin_prompt
from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import gather_diff, gather_docs
from loopflow.lf.launcher import build_claude_command, build_codex_command
from loopflow.lf.logging import get_model_env


@dataclass
class CommitMessage:
    """A commit/PR message with title and body."""

    title: str
    body: str


@dataclass
class ReleaseNotes:
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


def _log_cli_failure(action: str, error: Exception) -> None:
    msg = f"[lf] {action} via CLI failed ({type(error).__name__}): {error}."
    print(msg, file=sys.stderr)


def _log_success(action: str) -> None:
    if not _commit_debug_enabled():
        return
    print(f"[lf] {action} via CLI ok", file=sys.stderr)


def _normalize_json_newlines(text: str) -> str:
    """Replace actual newlines inside JSON strings with escaped \\n."""

    def escape_string_content(match: re.Match) -> str:
        content = match.group(1)
        # Replace actual newlines with escaped version
        content = content.replace("\n", "\\n")
        return f'"{content}"'

    # Match JSON string values (handle escaped quotes inside)
    return re.sub(r'"((?:[^"\\]|\\.)*)"', escape_string_content, text)


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

    # Look for ```json fence first to avoid matching {placeholders} in prose
    search_start = 0
    json_fence = text.find("```json")
    if json_fence != -1:
        search_start = json_fence

    start = text.find("{", search_start)
    end = text.rfind("}")
    if start == -1 or end == -1 or start >= end:
        return None
    candidate = text[start : end + 1]
    try:
        payload = json.loads(candidate)
    except json.JSONDecodeError:
        # Try normalizing newlines inside strings (Claude sometimes outputs actual newlines)
        try:
            normalized = _normalize_json_newlines(candidate)
            payload = json.loads(normalized)
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
        env=get_model_env(strip_api_keys=False),
    )
    output = result.stdout.strip() if result.stdout else ""
    if result.returncode != 0 or not output:
        detail = result.stderr.strip() if result.stderr else ""
        if not detail:
            detail = result.stdout.strip() if result.stdout else "CLI failed"
        raise RuntimeError(detail)
    return _parse_cli_message(output)


def _generate_message(repo_root: Path, prompt: str, action: str) -> CommitMessage:
    """Generate a message via CLI agent."""
    cli_prompt = prompt + "\n\nReturn JSON with keys: title, body. No extra text."
    try:
        message = _generate_message_via_cli(repo_root, cli_prompt)
        _log_success(action)
        return message
    except Exception as e:
        _log_cli_failure(action, e)
        raise


def generate_commit_message(repo_root: Path) -> CommitMessage:
    """Generate commit message for staged changes."""
    diff = _get_staged_diff(repo_root)
    task_prompt = get_builtin_prompt("commit_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "commit message")


def generate_commit_message_from_diff(repo_root: Path, diff: str | None) -> CommitMessage:
    """Generate commit message for a provided diff."""
    task_prompt = get_builtin_prompt("commit_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "commit message")


def generate_pr_message(repo_root: Path) -> CommitMessage:
    """Generate PR title and body from the branch diff."""
    diff = gather_diff(repo_root)
    task_prompt = get_builtin_prompt("pr_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "pr message")


def generate_pr_message_from_diff(repo_root: Path, diff: str | None) -> CommitMessage:
    """Generate PR title and body from a provided diff."""
    task_prompt = get_builtin_prompt("pr_message")
    prompt = _build_message_prompt(repo_root, diff, task_prompt)
    return _generate_message(repo_root, prompt, "pr message")


def _get_commits_since_tag(repo_root: Path, tag: str, full: bool = False) -> str | None:
    """Get commit log since a tag. If full=True, include commit bodies."""
    if full:
        # Full messages: subject + body, separated by blank lines
        fmt = "%s%n%b"
    else:
        fmt = "%s"
    result = subprocess.run(
        ["git", "log", f"{tag}..HEAD", f"--format={fmt}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return result.stdout


def _get_base_tag_for_release(old_version: str, new_version: str) -> str:
    """Determine the base tag for release notes based on bump type.

    For patch bumps: use old_version (e.g., 0.6.5 → 0.6.6 uses v0.6.5)
    For minor bumps: use first release of old minor (e.g., 0.6.11 → 0.7.0 uses v0.6.0)
    For major bumps: use first release of old major (e.g., 0.6.5 → 1.0.0 uses v0.0.0)
    """
    old_parts = [int(x) for x in old_version.split(".")]
    new_parts = [int(x) for x in new_version.split(".")]

    if len(old_parts) != 3 or len(new_parts) != 3:
        return f"v{old_version}"

    old_major, old_minor, _ = old_parts
    new_major, new_minor, _ = new_parts

    # Major bump: summarize all changes since start of old major version
    if new_major > old_major:
        return f"v{old_major}.0.0"

    # Minor bump: summarize all changes since start of old minor version
    if new_minor > old_minor:
        return f"v{old_major}.{old_minor}.0"

    # Patch bump: just changes since old version
    return f"v{old_version}"


def _parse_release_notes(output: str) -> ReleaseNotes:
    """Parse release notes from CLI output."""
    payload = _extract_json_payload(output)
    if payload and "summary" in payload and "changes" in payload:
        return ReleaseNotes(summary=payload["summary"], changes=payload["changes"])

    # Fallback: first line is summary, rest are changes
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    if not lines:
        raise ValueError("Empty release notes output")
    summary = lines[0]
    changes = lines[1:] if len(lines) > 1 else []
    return ReleaseNotes(summary=summary, changes=changes)


def generate_release_notes(repo_root: Path, old_version: str, new_version: str) -> ReleaseNotes:
    """Generate release notes from commits since last tag via CLI."""
    base_tag = _get_base_tag_for_release(old_version, new_version)

    # For minor/major releases, include full commit messages
    is_minor_or_major = base_tag != f"v{old_version}"
    commits = _get_commits_since_tag(repo_root, base_tag, full=is_minor_or_major)
    task_prompt = get_builtin_prompt("release_notes")

    parts = [f"Version bump: {old_version} → {new_version}"]
    if is_minor_or_major:
        release_type = "major" if new_version.split(".")[0] > old_version.split(".")[0] else "minor"
        parts.append(f"This is a {release_type} release summarizing all changes since {base_tag}.")
        parts.append("Use git diff and read files to understand the full impact of these changes.")
    if commits:
        parts.append(f"<commits>\n{commits}\n</commits>")
    parts.append(f"<task>\n{task_prompt}\n</task>")
    parts.append("\nReturn JSON with keys: summary, changes. No extra text.")
    prompt = "\n\n".join(parts)

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
        env=get_model_env(strip_api_keys=False),
    )
    output = result.stdout.strip() if result.stdout else ""
    if result.returncode != 0 or not output:
        detail = result.stderr.strip() if result.stderr else ""
        if not detail:
            detail = result.stdout.strip() if result.stdout else "CLI failed"
        raise RuntimeError(f"release notes generation failed: {detail}")

    _log_success("release notes")
    return _parse_release_notes(output)
