#!/usr/bin/env python3
"""Disposable `agy` for fixtures: one headless stream-json turn per process.

It prints what `agy` 1.5.13 prints. The model `no-such-model` is refused as an
unknown model is; a file `agy-refusal` beside `bin/` refuses every run with its
text, for a spent quota or a signed-out login.
"""

import json
import sys
import uuid
from pathlib import Path

HOME = Path(__file__).resolve().parent.parent
STEPS = HOME / "agy-steps.json"


def _emit(event, **body):
    print(json.dumps({"event": event, **body}), flush=True)


def _option(name):
    args = sys.argv[1:]
    return args[args.index(name) + 1] if name in args else None


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


def main():
    line = sys.stdin.readline()
    prompt = json.loads(line)["message"]["content"] if line.strip() else ""
    (HOME / "received.json").write_text(
        json.dumps({"args": sys.argv[1:], "input": prompt, "cwd": str(Path.cwd())})
    )
    refusal = HOME / "agy-refusal"
    if refusal.exists():
        _refuse(refusal.read_text().strip())
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
    update(step + 1, "ACTIVE", "agent_response", text_delta=answer)
    update(step + 1, "DONE", "agent_response", text_delta="\n")
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
    STEPS.write_text(json.dumps(steps))


if __name__ == "__main__":
    main()
