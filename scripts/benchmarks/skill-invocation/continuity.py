"""Probe native skill receipts and conversation continuity on installed providers."""

import argparse
import asyncio
import json
import os
import shlex
import shutil
import signal
import sys
import tempfile
import uuid
from contextlib import asynccontextmanager
from pathlib import Path

from probe import _environment, _read_output


@asynccontextmanager
async def _provider(command: list[str], root: Path):
    process = await asyncio.create_subprocess_exec(
        *command,
        cwd=root,
        env=_environment(),
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.DEVNULL,
        start_new_session=True,
        limit=4 * 1024 * 1024,
    )
    try:
        yield process
    finally:
        process.stdin.close()
        try:
            await asyncio.wait_for(process.wait(), 5)
        except TimeoutError:
            os.killpg(process.pid, signal.SIGKILL)
            await process.wait()


async def _send(process, message: dict) -> None:
    process.stdin.write((json.dumps(message) + "\n").encode())
    await process.stdin.drain()


async def _until(process, matches, events: list[dict]) -> dict:
    async with asyncio.timeout(55):
        while line := await process.stdout.readline():
            event = json.loads(line)
            events.append(event)
            if matches(event):
                return event
            if "method" in event and "id" in event:
                raise RuntimeError(f"unexpected provider request: {event['method']}")
    raise RuntimeError("provider closed before the expected receipt")


async def _request(process, method: str, params: dict, events: list[dict]) -> dict:
    request_id = uuid.uuid4().hex
    await _send(process, {"id": request_id, "method": method, "params": params})
    reply = await _until(process, lambda event: event.get("id") == request_id, events)
    if "error" in reply:
        raise RuntimeError(f"{method}: {reply['error'].get('message')}")
    return reply["result"]


def _emit(case: str, *, evidence: dict, **facts) -> bool:
    print(json.dumps({"case": case, **facts, "evidence": evidence}), flush=True)
    return all(value is True for value in facts.values())


def _input_receipt(expected: dict, content: list[dict]) -> bool:
    return any(all(item.get(key) == value for key, value in expected.items()) for item in content)


def _codex_receipts(history: list[dict], turn_id: str, skill: Path, inputs: list[dict]) -> dict:
    messages = [
        event["payload"]
        for event in history
        if event.get("type") == "response_item"
        and event["payload"].get("role") == "user"
        and event["payload"].get("internal_chat_message_metadata_passthrough", {}).get("turn_id")
        == turn_id
    ]
    texts = [block.get("text", "") for message in messages for block in message["content"]]
    return {
        "expanded_skill": any(
            f"<path>{skill}</path>" in text and skill.read_text() in text for text in texts
        ),
        "native_invocation": inputs[0]["text"] in texts,
        "native_context": inputs[2]["text"] in texts,
    }


def _claude_receipts(history: list[dict], skill: Path, argument: str, context: str) -> dict:
    users = [event["message"]["content"] for event in history if event.get("type") == "user"]
    expanded = [
        block.get("text", "")
        for content in users
        if isinstance(content, list)
        for block in content
        if block.get("type") == "text"
    ]
    attachments = [
        event
        for event in history
        if event.get("type") == "attachment"
        and event.get("attachment", {}).get("type") == "hook_additional_context"
        and context in "\n".join(event["attachment"]["content"])
    ]
    return {
        "selected_source": any(
            f"Base directory for this skill: {skill.parent}\n" in text and f"|{argument}|" in text
            for text in expanded
        ),
        "separate_context": bool(attachments),
        "context_roles": [event.get("renderedRole") for event in attachments],
        "context_attachment_ids": [event["uuid"] for event in attachments],
    }


async def _codex(codex: str, root: Path) -> bool:
    name = "lf-continuity-" + uuid.uuid4().hex[:12]
    marker, context = uuid.uuid4().hex, uuid.uuid4().hex
    skill = root / ".agents" / "skills" / name / "SKILL.md"
    skill.parent.mkdir(parents=True)
    skill.write_text(
        f"---\nname: {name}\ndescription: Native continuity fixture.\n---\n"
        f"Reply with {marker}, the invocation argument, and the context marker. "
        "Do not use tools.\n"
    )
    command = [
        codex,
        "app-server",
        "--listen",
        "stdio://",
        "-c",
        "mcp_servers={}",
        "-c",
        "features.apps=false",
        "-c",
        "features.plugins=false",
    ]
    thread_id = None
    first_turn = None
    passed = True
    for argument in ["alpha", "beta"]:
        events = []
        async with _provider(command, root) as process:
            await _request(
                process,
                "initialize",
                {
                    "clientInfo": {"name": "lf_skill_probe", "version": "1"},
                    "capabilities": {"experimentalApi": True},
                },
                events,
            )
            await _send(process, {"method": "initialized"})
            params = {"cwd": str(root), "approvalPolicy": "never", "sandbox": "read-only"}
            if thread_id:
                params["threadId"] = thread_id
            response = await _request(
                process, "thread/resume" if thread_id else "thread/start", params, events
            )
            resumed_id = response["thread"]["id"]
            same_thread = thread_id is None or resumed_id == thread_id
            thread_id = resumed_id
            invocation = f"${name} {argument}"
            inputs = [
                {"type": "text", "text": invocation},
                {"type": "skill", "name": name, "path": str(skill)},
                {"type": "text", "text": f"<lf:context>Context marker: {context}</lf:context>"},
            ]
            turn = await _request(
                process,
                "turn/start",
                {
                    "threadId": thread_id,
                    "input": inputs,
                },
                events,
            )
            turn_id = turn["turn"]["id"]
            completed = await _until(
                process,
                lambda event: (
                    event.get("method") == "turn/completed"
                    and event["params"]["turn"]["id"] == turn_id
                ),
                events,
            )
            history = await _request(
                process,
                "thread/read",
                {
                    "threadId": thread_id,
                    "includeTurns": True,
                },
                events,
            )
            turns = history["thread"]["turns"]
            native_path = Path(history["thread"]["path"])
            native = _codex_receipts(
                [json.loads(line) for line in native_path.read_text().splitlines()],
                turn_id,
                skill,
                inputs,
            )
            current = next(value for value in turns if value["id"] == turn_id)
            users = [item for item in current["items"] if item["type"] == "userMessage"]
            content = [item for user in users for item in user["content"]]
            answer = "\n".join(
                item.get("text", "") for item in current["items"] if item["type"] == "agentMessage"
            )
            # A returned skill item proves selection; a model marker alone does not.
            passed &= _emit(
                "codex_app_server_" + argument,
                completed=completed["params"]["turn"]["status"] == "completed",
                same_thread=same_thread,
                retained_turns=len(turns) == (1 if argument == "alpha" else 2),
                prior_turn_unchanged=first_turn is None or turns[0] == first_turn,
                invocation_receipt=_input_receipt(inputs[0], content),
                skill_receipt=_input_receipt(inputs[1], content),
                separate_context_receipt=_input_receipt(inputs[2], content),
                **native,
                answer_skill=marker in answer,
                answer_argument=argument in answer,
                answer_context=context in answer,
                evidence={
                    "thread_id": thread_id,
                    "turn_id": turn_id,
                    "skill": str(skill),
                    "transcript": str(native_path),
                },
            )
            first_turn = turns[0]
    return passed


async def _claude(claude: str, root: Path) -> bool:
    name = "lf-continuity-" + uuid.uuid4().hex[:12]
    marker, context = uuid.uuid4().hex, uuid.uuid4().hex
    session_id = str(uuid.uuid4())
    skill = root / ".claude" / "skills" / name / "SKILL.md"
    skill.parent.mkdir(parents=True)
    skill.write_text(
        f"---\nname: {name}\ndescription: Native continuity fixture.\n"
        "disable-model-invocation: true\n---\n"
        f"Reply with {marker}|$ARGUMENTS|. If the argument is alpha, do not repeat "
        "the context marker anywhere in your answer or reasoning. Otherwise append the "
        "context marker. Do not use tools.\n"
    )
    hook = root / "hook.py"
    hook_source = (
        "import json, sys\nfrom pathlib import Path\n"
        "root = Path(__file__).parent\n"
        "event = json.load(sys.stdin)\n"
        "with (root / 'hooks.jsonl').open('a') as f:\n"
        "    f.write(json.dumps(event) + '\\n')\n"
        f"if event['session_id'] == {session_id!r} and event['prompt'].startswith('/{name} '):\n"
        "    print(json.dumps({'hookSpecificOutput': {'hookEventName': 'UserPromptSubmit', "
        f"'additionalContext': '<lf:context>Context marker: {context}</lf:context>'}}}}))\n"
    )
    hook.write_text(hook_source)
    settings = json.dumps(
        {
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": shlex.join([sys.executable, str(hook)]),
                            }
                        ]
                    }
                ]
            }
        }
    )
    base = [
        claude,
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
    passed = True
    for argument, flags in [
        ("alpha", ["--session-id", session_id, "--settings", settings]),
        ("beta", ["--resume", session_id]),
        ("gamma", ["--resume", session_id, "--settings", settings]),
    ]:
        if argument == "beta":
            hook.unlink()
        elif argument == "gamma":
            hook.write_text(hook_source)
        events = []
        invocation = f"/{name} {argument}"
        async with _provider(base + flags, root) as process:
            await _send(
                process,
                {
                    "type": "user",
                    "message": {
                        "role": "user",
                        "content": [{"type": "text", "text": invocation}],
                    },
                },
            )
            result = await _until(process, lambda event: event.get("type") == "result", events)
        text, native_argument = _read_output("\n".join(map(json.dumps, events)), "claude")
        receipts = [json.loads(line) for line in (root / "hooks.jsonl").read_text().splitlines()]
        native_path = Path(receipts[0]["transcript_path"])
        history = [json.loads(line) for line in native_path.read_text().splitlines()]
        native = _claude_receipts(history, skill, argument, context)
        replay = [
            event for event in events if event.get("type") == "user" and event.get("isReplay")
        ]
        replay_text = json.dumps(replay)
        passed &= _emit(
            "claude_resume_" + argument,
            completed=result.get("is_error") is False,
            same_session=result.get("session_id") == session_id,
            exact_arguments=native_argument == argument,
            skill_receipt=f"<command-name>/{name}</command-name>" in replay_text,
            selected_source=native["selected_source"],
            separate_context_receipt=native["separate_context"],
            context_attachment_count=len(native["context_attachment_ids"])
            == (2 if argument == "gamma" else 1),
            hook_invoked_when_supplied=any(event["prompt"] == invocation for event in receipts)
            == (argument != "beta"),
            answer_skill=marker in text,
            answer_argument=f"|{argument}|" in text,
            answer_context=(context in text) == (argument != "alpha"),
            no_earlier_answer_leak=argument != "alpha"
            or not any(
                context in json.dumps(event["message"])
                for event in history
                if event.get("type") == "assistant"
            ),
            evidence={
                "session_id": session_id,
                "transcript": str(native_path),
                "context_roles": native["context_roles"],
                "context_attachment_ids": native["context_attachment_ids"],
            },
        )
    return passed


async def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider", choices=["claude", "codex"], required=True)
    parser.add_argument("--executable", required=True)
    args = parser.parse_args()
    executable = shutil.which(args.executable)
    if executable is None:
        parser.error("provider executable not found")
    version = await asyncio.create_subprocess_exec(
        executable,
        "--version",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.DEVNULL,
        env=_environment(),
    )
    stdout, _ = await asyncio.wait_for(version.communicate(), 10)
    print(json.dumps({"executable": executable, "version": stdout.decode().strip()}), flush=True)
    with tempfile.TemporaryDirectory(prefix="lf-skill-continuity-") as temporary:
        root = Path(temporary).resolve()
        try:
            passed = await (_claude if args.provider == "claude" else _codex)(executable, root)
        except (RuntimeError, TimeoutError) as error:
            print(json.dumps({"provider": args.provider, "error": str(error) or "timeout"}))
            return 1
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(asyncio.run(_main()))
