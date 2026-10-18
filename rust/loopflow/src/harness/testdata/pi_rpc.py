# Isolated Pi 1.3 protocol stand-in; no provider, credential or network access.
import json
import os
import subprocess
import sys
from pathlib import Path

if "--version" in sys.argv:
    print("1.3.0")
    sys.exit(0)

args = sys.argv[1:]
file = (
    Path(args[args.index("--session") + 1])
    if "--session" in args
    else Path.cwd() / "pi-conversation.jsonl"
)
if not file.exists():
    file.write_text('{"type":"session","id":"fixture","version":3}\n')


def _emit(value):
    print(json.dumps(value), flush=True)


def _message(text):
    return {
        "type": "message_end",
        "message": {
            "role": "assistant",
            "content": [{"type": "text", "text": text}],
            "stopReason": "stop",
            "model": "fixture/model",
            "usage": {
                "input": 10,
                "output": 3,
                "cacheRead": 2,
                "cacheWrite": 1,
                "cost": {"total": 0.01},
            },
        },
    }


def _record(text):
    if not os.environ.get("PI_FIXTURE_RECORD"):
        return
    record = {
        "args": args,
        "cwd": str(Path.cwd()),
        "input": text,
        "caller": json.loads(os.environ["LF_AGENT_CALLER"]),
        "lf_bin": os.environ["LF_BIN"],
    }
    if "--append-system-prompt" in args:
        record["system"] = Path(args[args.index("--append-system-prompt") + 1]).read_text()
    if os.environ.get("LF_CONTEXT_DELIVERY"):
        output = subprocess.run(
            [os.environ["LF_BIN"], "__context-block", "--moment", "start"],
            input=json.dumps(
                {
                    "session_id": str(file),
                    "hook_event_name": "PiContext",
                    "adapter_invocation_id": "fixture",
                }
            ),
            text=True,
            capture_output=True,
            check=True,
        )
        record["context"] = json.loads(output.stdout)["hookSpecificOutput"]["additionalContext"]
    if os.environ.get("PI_FIXTURE_NESTED"):
        nested = subprocess.run(
            [os.environ["LF_BIN"], "context"], text=True, capture_output=True, check=True
        )
        record["nested_context"] = nested.stdout
    Path(os.environ["PI_FIXTURE_RECORD"]).write_text(json.dumps(record))


if "--mode" not in args:

    def _callback(event):
        event = (
            dict(sessionFile=str(file), turn="native-fixture-turn", openTools=0, pendingInput=0)
            | event
        )
        subprocess.run(
            [os.environ["LF_BIN"], "__pi-event"], input=json.dumps(event), text=True, check=True
        )

    _callback({"type": "session_start"})
    _callback({"type": "ui_prompt_start", "turn": None, "pendingInput": 1})
    _callback({"type": "ui_prompt_end", "turn": None})
    _record(args[-1] if "--" in args else "")
    _callback({"type": "agent_start"})
    events = [_message("Checking")]
    _callback(events[-1] | {"events": events})
    _callback(events[-1] | {"events": events})  # repeated observation is idempotent
    if os.environ.get("PI_FIXTURE_CRASH"):
        sys.exit(9)
    events.append(_message("native answer"))
    _callback(events[-1] | {"events": events})
    _callback({"type": "agent_settled", "aborted": False, "events": events})
    sys.exit(0)


for line in sys.stdin:
    request = json.loads(line)
    command = request["type"]
    response = {"type": "response", "command": command, "id": request.get("id"), "success": True}
    if command == "get_state":
        response["data"] = {"sessionFile": str(file), "sessionId": "fixture"}
    elif command == "steer":
        text = request["message"]
        if text == "reject":
            response.update(success=False, error="Steer rejected")
        elif text == "malformed":
            del response["success"]
            response["data"] = {"disposition": "queued"}
        elif text == "disconnect":
            sys.exit(9)
        else:
            if text == "late":
                _emit({"type": "agent_settled", "aborted": True})
            response["data"] = {"disposition": "handled" if text == "handled" else "queued"}
    elif command == "prompt":
        text = request["message"]
        _record(text)
        if text == "reject":
            response.update(success=False, error="Model not found: fixture")
        else:
            response["data"] = {"disposition": "started"}
    _emit(response)
    if command == "steer" and text == "finish":
        _emit(_message("steering incorporated"))
        _emit({"type": "agent_settled", "aborted": False})
    if command == "abort":
        _emit({"type": "agent_settled", "aborted": True})
    if command != "prompt" or not response["success"]:
        continue
    if text == "crash":
        _emit(
            {
                "type": "tool_execution_start",
                "toolCallId": "open",
                "toolName": "bash",
                "args": {"command": "read"},
            }
        )
        sys.exit(9)
    if text == "invalid":
        print("not-json", flush=True)
        continue
    if text == "dialog":
        _emit({"type": "extension_ui_request", "method": "confirm", "id": "dialog"})
        continue
    if text == "extension error":
        _emit({"type": "extension_error", "error": "fixture context delivery failed"})
        continue
    if text == "wait":
        _emit({"type": "agent_start"})
        continue
    if text == "auth":
        _emit(
            {
                "type": "message_end",
                "message": {
                    "role": "assistant",
                    "content": [],
                    "stopReason": "error",
                    "errorMessage": "No API key configured",
                },
            }
        )
        _emit({"type": "agent_settled", "aborted": False})
        continue
    if text == "tool":
        _emit(_message("Checking"))
        _emit(
            {
                "type": "tool_execution_start",
                "toolCallId": "tool",
                "toolName": "bash",
                "args": {"command": "pwd"},
            }
        )
        _emit(
            {
                "type": "tool_execution_end",
                "toolCallId": "tool",
                "toolName": "bash",
                "result": {"content": [{"type": "text", "text": str(Path.cwd())}]},
                "isError": False,
            }
        )
    with file.open("a") as saved:
        saved.write(json.dumps({"prompt": text}) + "\n")
    count = len(file.read_text().splitlines()) - 1
    answer = '{"path":"alpha"}' if "PI_ROUTE_SENTINEL" in text else f"answer {count}: {text}"
    _emit(_message(answer))
    _emit({"type": "agent_end", "messages": [], "willRetry": False})
    _emit({"type": "agent_settled", "aborted": False})
