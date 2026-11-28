#!/usr/bin/env python3
"""Disposable `agy` for fixtures: one headless stream-json turn per process,
or, with no `--output-format`, a terminal whose person sends `-i` and leaves.

It prints what `agy` 1.5.13 prints and runs the hooks in its `--add-dir`
folder's `.agents/hooks.json` as that version does. The model `no-such-model`
is refused as an unknown model is. Each turn is appended to the conversation's
`transcript_full.jsonl` under the home, where lf reads final answers and usage.

A file `hold-turn` beside `bin/` keeps a headless turn running, with `holding`
written once it is under way, until the file is removed.

A fixture that leaves `tool_turn.py` beside `bin/` gets a turn that runs one
tool and a subagent, and reads, in order: `hook-context` (what the first
model call's hook put in front of the model), `holding` (the tool is running)
and, once it writes `tool-done`, `hook-steer` (what the next model call's hook
put there).
"""

import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

HOME = Path(__file__).resolve().parent.parent
STEPS = HOME / "agy-steps.json"


def _emit(event, **body):
    print(json.dumps({"event": event, **body}), flush=True)


def _option(name):
    args = sys.argv[1:]
    return args[args.index(name) + 1] if name in args else None


def _hook(event, conversation, **more):
    """Run the hooks declared for `event`; the text they put in front of the model."""
    folder = _option("--add-dir")
    declared = Path(folder or "", ".agents", "hooks.json")
    if not folder or not declared.exists():
        return ""
    payload = {
        "conversationId": conversation,
        "hookEventName": event,
        "cwd": str(Path.cwd()),
        "workspacePaths": [str(Path.cwd()), folder],
        "modelName": _option("--model") or "fixture",
        # A person's text, escaped in the payload; a subagent's is empty.
        "lastUserInput": 'say "invocationNum":3',
        **more,
    }
    texts = []
    for hooks in json.loads(declared.read_text()).values():
        for hook in hooks.get(event, []):
            output = subprocess.run(
                hook["command"],
                shell=True,
                input=json.dumps(payload, separators=(",", ":")),
                env=os.environ | {"ANTIGRAVITY_CONVERSATION_ID": conversation},
                cwd=declared.parent,
                text=True,
                capture_output=True,
                timeout=60,
                check=True,
            ).stdout
            messages = [step["systemMessage"] for step in json.loads(output).get("injectSteps", [])]
            # agy 1.5.16 and 1.5.17 discard this hook's entire reply, including
            # ordinary context steps beside the one requesting inline markdown.
            if any(isinstance(m.get("metadata", {}).get("user_markdown"), str) for m in messages):
                continue
            texts.extend(m["systemMessage"] for m in messages)
    return "\n".join(texts)


def _keep(conversation, rows):
    """Append a turn's rows to the transcript `agy` keeps for the conversation."""
    logs = Path.home() / ".gemini/antigravity-cli/brain" / conversation / ".system_generated/logs"
    logs.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    with (logs / "transcript_full.jsonl").open("a") as transcript:
        for index, kind, more in rows:
            row = {"step_index": index, "type": kind, "status": "DONE", "created_at": stamp}
            transcript.write(json.dumps(row | more) + "\n")


def _call(input_tokens, output_tokens, **more):
    return {
        "source": "MODEL",
        "model": _option("--model") or "fixture",
        "input_tokens": input_tokens,
        "cache_read_tokens": 0,
        "output_tokens": output_tokens,
        **more,
    }


def _refuse(error):
    _emit(
        "result",
        result={
            "conversation_id": "",
            "status": "ERROR",
            "response": "",
            "error": error,
            "num_turns": 0,
            "finished_reason": "error",
        },
    )
    print(f"error: {error}", file=sys.stderr)
    sys.exit(1)


def _receive(prompt):
    (HOME / "received.json").write_text(
        json.dumps({"args": sys.argv[1:], "input": prompt, "cwd": str(Path.cwd())})
    )


def _wait(released, what):
    deadline = time.monotonic() + 120
    while not released():
        if time.monotonic() >= deadline:
            raise TimeoutError(f"the fixture's {what} was not released")
        time.sleep(0.02)


def _terminal():
    """The TUI: the `-i` prompt is one turn, then the person leaves. A fixture's
    `tui-new` beside `bin/` has them type `/new` and its text first, which
    continues the same process under a second conversation."""
    prompt = _option("-i") or ""
    _receive(prompt)
    resumed = _option("--conversation")
    turns = [(resumed or str(uuid.uuid4()), prompt, bool(resumed))] if prompt else []
    if (HOME / "tui-new").exists():
        turns.append((str(uuid.uuid4()), (HOME / "tui-new").read_text(), False))
    heard = []
    for conversation, prompt, resumed_thread in turns:
        heard.append(
            {
                "conversation": conversation,
                "context": _hook(
                    "PreInvocation", conversation, lastUserInput=prompt,
                    invocationNum=0, initialNumSteps=3 if resumed_thread else 1,
                ),
            }
        )
        request = f"<USER_REQUEST>\n{prompt}\n</USER_REQUEST>"
        rows = [
            (0, "USER_INPUT", {"source": "USER_EXPLICIT", "content": request}),
            (1, "PLANNER_RESPONSE", _call(20, 5, content=f"answer to: {prompt}\n")),
        ]
        _keep(conversation, rows)
        _hook("Stop", conversation, executionNum=0, terminationReason="NO_TOOL_CALL")
    (HOME / "tui-heard.json").write_text(json.dumps(heard))


def main():
    if "--output-format" not in sys.argv:
        return _terminal()
    line = sys.stdin.readline()
    prompt = json.loads(line)["message"]["content"] if line.strip() else ""
    _receive(prompt)
    model = _option("--model")
    if model == "no-such-model":
        _refuse(
            f'invalid model selection (--model "{model}" --effort ""): model {model} is not'
            " recognized as a known model or custom model in settings\n"
            "Available models:\n  Gemini 3.8 Flash (High)"
        )

    steps = json.loads(STEPS.read_text()) if STEPS.exists() else {}
    conversation = _option("--conversation") or str(uuid.uuid4())
    step = steps.get(conversation, 0)
    answer = f"answer to: {prompt}"

    def update(index, state, kind, **more):
        _emit(
            "step_update",
            step_update={
                "conversation_id": conversation,
                "step_index": index,
                "state": state,
                "step_type": kind,
                **more,
            },
        )

    _emit(
        "init",
        conversation_id=conversation,
        init={"cwd": str(Path.cwd()), "tools": ["run_command"], "model": model},
    )
    update(step, "DONE", "user_input")
    request = f"<USER_REQUEST>\n{prompt}\n</USER_REQUEST>"
    kept = [(step, "USER_INPUT", {"source": "USER_EXPLICIT", "content": request})]
    tokens = (20, 5)
    context = _hook("PreInvocation", conversation, invocationNum=0, initialNumSteps=step + 1)
    # A fixture's `hold-turn` keeps the model call running until it is removed.
    if (HOME / "hold-turn").exists():
        (HOME / "holding").write_text(conversation)
    _wait(lambda: not (HOME / "hold-turn").exists(), "turn")
    if (HOME / "ask-question").exists():
        tool = {"name": "ask_question", "parameters": {"Question": "Use the fixture?"}}
        step += 1
        for state in ("ACTIVE", "DONE"):
            update(step, state, "tool", tool_name="ask_question", tool_info=tool)
    if (HOME / "tool_turn.py").exists():
        (HOME / "hook-context").write_text(context)
        tool = {"name": "run_command", "parameters": {"CommandLine": "echo hi"}}
        # A model call that only calls a tool prints `DONE` with no text.
        update(step + 1, "DONE", "agent_response")
        calls = [{"name": tool["name"], "args": tool["parameters"]}]
        kept.append((step + 1, "PLANNER_RESPONSE", _call(12, 2, tool_calls=calls)))
        step += 1
        update(step + 1, "ACTIVE", "tool", tool_name="run_command", tool_info=tool)
        (HOME / "holding").write_text(conversation)
        _wait((HOME / "tool-done").exists, "tool")
        update(step + 1, "DONE", "tool", tool_name="run_command", tool_info=tool)
        kept.append((step + 1, "GENERIC", {"source": "MODEL", "content": "hi\n"}))
        tokens = (8, 3)
        # A subagent runs the same hooks under its own conversation.
        heard = _hook(
            "PreInvocation", str(uuid.uuid4()), invocationNum=0, initialNumSteps=0, lastUserInput=""
        )
        if heard:
            raise AssertionError(f"a subagent heard the launch's news: {heard}")
        steer = _hook("PreInvocation", conversation, invocationNum=1, initialNumSteps=step + 2)
        (HOME / "hook-steer").write_text(steer)
        step += 1
    update(step + 1, "ACTIVE", "agent_response", text_delta=answer)
    update(step + 1, "DONE", "agent_response", text_delta="\n")
    kept.append((step + 1, "PLANNER_RESPONSE", _call(*tokens, content=answer + "\n")))
    _keep(conversation, kept)
    _hook("Stop", conversation, executionNum=0, terminationReason="NO_TOOL_CALL")
    _emit(
        "result",
        result={
            "conversation_id": conversation,
            "status": "SUCCESS",
            "response": answer + "\n",
            "num_turns": 1,
        },
    )
    steps[conversation] = step + 2
    # Whole or not at all: a Task's conversation and its worker launch at once.
    partial = STEPS.with_name(f"{STEPS.name}.{os.getpid()}")
    partial.write_text(json.dumps(steps))
    partial.replace(STEPS)


if __name__ == "__main__":
    main()
