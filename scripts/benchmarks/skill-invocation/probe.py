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
from pathlib import Path


def _environment() -> dict[str, str]:
    return {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("LF_", "LOOPFLOW_", "CLAUDECODE", "CLAUDE_CODE_"))
    }


def _run(
    command: list[str], root: Path, input_text: str | None = None
) -> tuple[subprocess.CompletedProcess[str] | None, float]:
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
        return None, round(time.monotonic() - started, 3)
    return result, round(time.monotonic() - started, 3)


def _result_text(output: str, provider: str) -> str:
    texts = []
    for line in output.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if provider == "claude" and event.get("type") == "result":
            texts.append(event.get("result", ""))
        elif event.get("type") == "item.completed":
            item = event.get("item", {})
            if item.get("type") == "agent_message":
                texts.append(item.get("text", ""))
    return "\n".join(texts)


def _observation(
    case: str,
    provider: str,
    command: list[str],
    root: Path,
    marker: str,
    context_marker: str,
    input_text: str | None = None,
) -> dict[str, object]:
    result, seconds = _run(command, root, input_text)
    text = _result_text(result.stdout, provider) if result else ""
    native_arguments = None
    if result and provider == "claude":
        for line in result.stdout.splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if event.get("type") != "user" or not event.get("isReplay"):
                continue
            content = event.get("message", {}).get("content")
            if isinstance(content, str):
                match = re.search(r"<command-args>(.*?)</command-args>", content, re.DOTALL)
                if match:
                    native_arguments = match.group(1)
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
        base = [
            claude,
            "-p",
            "--no-session-persistence",
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
        settings = json.dumps(
            {
                "hooks": {
                    "UserPromptSubmit": [
                        {
                            "hooks": [
                                {
                                    "type": "command",
                                    "command": f"cat {shlex.quote(str(hook_file))}",
                                }
                            ]
                        }
                    ]
                }
            }
        )
        for case, blocks, flags in [
            ("claude_plain", [invocation], []),
            ("claude_context_prefix", [f"{context}\n\n{invocation}"], []),
            ("claude_context_suffix", [f"{invocation}\n\n{context}"], []),
            ("claude_separate_blocks", [invocation, context], []),
            ("claude_context_hook", [invocation], ["--settings", settings]),
        ]:
            message = {
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [{"type": "text", "text": block} for block in blocks],
                },
            }
            observation = _observation(
                case,
                "claude",
                base + flags,
                root,
                marker,
                context_marker,
                json.dumps(message) + "\n",
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
    required = ["claude_plain", "claude_context_hook"]
    passed = all(
        by_case[case]["exit"] == 0
        and by_case[case]["skill_marker"]
        and by_case[case]["native_arguments_exact"]
        for case in required
    ) and all(
        [
            by_case["claude_context_hook"]["context_marker"],
            by_case["codex_exec"]["exit"] == 0,
            by_case["codex_exec"]["skill_marker"],
            by_case["codex_exec"]["response_argument"],
        ]
    )
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
