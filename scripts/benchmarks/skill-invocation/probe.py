"""Probe native skills without changing provider settings or installed skills."""

import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import tempfile
import time
import uuid
from collections.abc import Iterable, Iterator
from pathlib import Path


def _environment() -> dict[str, str]:
    return {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("LF_", "LOOPFLOW_", "CLAUDECODE", "CLAUDE_CODE_"))
    }


def _claude_command(executable: str) -> list[str]:
    return [
        executable,
        "-p",
        "--setting-sources",
        "project",
        "--strict-mcp-config",
        "--mcp-config",
        '{"mcpServers":{}}',
        "--tools",
        "",
        "--output-format",
        "stream-json",
        "--input-format",
        "stream-json",
        "--verbose",
        "--replay-user-messages",
    ]


def _user_message(blocks: list[str]) -> dict:
    return {
        "type": "user",
        "message": {
            "role": "user",
            "content": [{"type": "text", "text": block} for block in blocks],
        },
    }


def _hook_settings(command: str) -> str:
    return json.dumps(
        {"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": command}]}]}}
    )


def _output_events(output: str) -> Iterator[dict]:
    for line in output.splitlines():
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            continue


def _read_output(events: Iterable[dict]) -> tuple[str, str | None]:
    texts = []
    native_arguments = None
    for event in events:
        if event.get("type") == "result":
            texts.append(event.get("result", ""))
        elif event.get("type") == "user" and event.get("isReplay"):
            content = event.get("message", {}).get("content")
            if isinstance(content, str):
                match = re.search(r"<command-args>(.*?)</command-args>", content, re.DOTALL)
                if match:
                    native_arguments = match.group(1)
        elif event.get("type") == "item.completed":
            item = event.get("item", {})
            if item.get("type") == "agent_message":
                texts.append(item.get("text", ""))
    return "\n".join(texts), native_arguments


def _observation(
    case: str,
    provider: str,
    command: list[str],
    root: Path,
    marker: str,
    context_marker: str,
    input_text: str | None = None,
) -> dict[str, object]:
    started = time.monotonic()
    try:
        result = subprocess.run(
            command,
            cwd=root,
            env=_environment(),
            input=input_text,
            stdin=subprocess.DEVNULL if input_text is None else None,
            capture_output=True,
            text=True,
            timeout=55,
        )
    except subprocess.TimeoutExpired:
        result = None
    seconds = round(time.monotonic() - started, 3)
    text, native_arguments = _read_output(_output_events(result.stdout if result else ""))
    return {
        "case": case,
        "exit": result.returncode if result else None,
        "timeout": result is None,
        "seconds": seconds,
        "skill_marker": marker in text,
        "response_argument": f"{marker}|alpha|" in text,
        "native_arguments_exact": native_arguments == "alpha" if provider == "claude" else None,
        "context_in_native_arguments": bool(
            native_arguments and context_marker in native_arguments
        ),
        "context_marker": context_marker in text,
    }


def _probe(claude: str, codex: str) -> list[dict[str, object]]:
    name = f"lf-probe-{uuid.uuid4().hex[:12]}"
    marker = uuid.uuid4().hex
    context_marker = uuid.uuid4().hex
    observations = []
    with tempfile.TemporaryDirectory(prefix="lf-skill-probe-") as temporary:
        root = Path(temporary)
        for folder, body, header in [
            (
                ".claude/skills",
                f"Reply with exactly {marker}|$ARGUMENTS| followed by the context "
                "marker if one was supplied, otherwise none. Do not use tools.\n",
                "disable-model-invocation: true\nallowed-tools: Read\n",
            ),
            (
                ".agents/skills",
                f"Reply with exactly {marker}| followed by the invocation's "
                "argument, then |none. Do not use tools.\n",
                "",
            ),
        ]:
            directory = root / folder / name
            directory.mkdir(parents=True)
            (directory / "SKILL.md").write_text(
                f"---\nname: {name}\ndescription: Return the native skill probe marker.\n"
                f"{header}---\n{body}"
            )

        context = f"<lf:context>The context marker is {context_marker}.</lf:context>"
        invocation = f"/{name} alpha"
        base = _claude_command(claude) + ["--no-session-persistence"]
        hook_file = root / "context.json"
        hook_file.write_text(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": context,
                    }
                }
            )
        )
        settings = _hook_settings(f"cat {shlex.quote(str(hook_file))}")
        for case, blocks, flags in [
            ("claude_plain", [invocation], []),
            ("claude_context_prefix", [f"{context}\n\n{invocation}"], []),
            ("claude_context_suffix", [f"{invocation}\n\n{context}"], []),
            ("claude_separate_blocks", [invocation, context], []),
            ("claude_context_hook", [invocation], ["--settings", settings]),
        ]:
            observation = _observation(
                case,
                "claude",
                base + flags,
                root,
                marker,
                context_marker,
                json.dumps(_user_message(blocks)) + "\n",
            )
            observations.append(observation)
            print(json.dumps(observation), flush=True)

        observation = _observation(
            "codex_exec",
            "codex",
            [
                codex,
                "exec",
                "--ignore-user-config",
                "--skip-git-repo-check",
                "--ephemeral",
                "--sandbox",
                "read-only",
                "--json",
                f"${name} alpha",
            ],
            root,
            marker,
            context_marker,
        )
        observations.append(observation)
        print(json.dumps(observation), flush=True)
    return observations


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", default=shutil.which("claude"))
    parser.add_argument("--codex", default=shutil.which("codex"))
    args = parser.parse_args()
    if not args.claude or not args.codex:
        parser.error("both provider executables are required")
    for provider in [args.claude, args.codex]:
        result = subprocess.run(
            [provider, "--version"],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            env=_environment(),
            timeout=10,
            check=True,
        )
        print(json.dumps({"executable": provider, "version": result.stdout.strip()}))
    observations = _probe(args.claude, args.codex)
    by_case = {observation["case"]: observation for observation in observations}
    passed = (
        all(
            by_case[case]["exit"] == 0 and by_case[case]["skill_marker"]
            for case in ("claude_plain", "claude_context_hook", "codex_exec")
        )
        and by_case["claude_plain"]["native_arguments_exact"]
        and by_case["claude_context_hook"]["native_arguments_exact"]
        and by_case["claude_context_hook"]["context_marker"]
        and by_case["codex_exec"]["response_argument"]
    )
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
