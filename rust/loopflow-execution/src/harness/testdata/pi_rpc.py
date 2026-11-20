# Isolated Pi 1.3 protocol stand-in; no provider, credential or network access.
import json
import os
import socket
import subprocess
import sys
import time
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
# A fixture leaves this marker to make any prompt open a dialog.
ASK = Path(os.environ.get("LF_HOME", ".")) / "ask-question"
# And this one to make any prompt start a turn that never ends on its own.
HOLD = Path(os.environ.get("LF_HOME", ".")) / "hold-turn"
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


def _append(entry_id, parent, message):
    # Pi appends each message to its session file, where lf reads usage and answers.
    stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    entry = dict(type="message", id=entry_id, parentId=parent, timestamp=stamp)
    with file.open("a") as transcript:
        transcript.write(json.dumps(entry | {"message": message}) + "\n")


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


def _tool_turn():
    """The extension's calls to lf around a tool, for a fixture that left `tool_turn.py`."""
    script = HOLD.with_name("tool_turn.py")
    if not script.exists():
        return
    extension = Path(args[args.index("--extension") + 1]).read_text()
    spec = json.loads(extension.split("\n", 1)[0].removeprefix("const spec = ").removesuffix(";"))
    block = [spec["executable"], "__context-block", "--moment"]
    hooks = {
        # The block is rendered without the launch's capability.
        "start": {
            "argv": [*block, "start"],
            "event": "PiContext",
            "without": ["LF_PROVIDER_CONTEXT", "LF_BARE_STEER"],
        },
        "steer": {"argv": [*block, "steer"], "event": "PiSteer"},
    }
    subprocess.run([sys.executable, str(script), str(file), json.dumps(hooks)], check=True)


if "--mode" not in args:
    # The extension's one Machine server connection, opened with the address
    # its launch spec (the source's first line) carries.
    extension = Path(args[args.index("--extension") + 1]).read_text()
    spec = json.loads(extension.split("\n", 1)[0].removeprefix("const spec = ").removesuffix(";"))
    connection = socket.socket(socket.AF_UNIX)
    connection.connect(spec["server"]["socket"])
    server = connection.makefile("rw")

    def _send(message):
        server.write(json.dumps(message) + "\n")
        server.flush()
        answer = json.loads(server.readline())
        if answer["type"] == "error":
            raise SystemExit(f"session.boundary refused: {answer['message']}")
        return answer

    _send(
        {
            "type": "hello",
            "revision": spec["server"]["revision"],
            "min_revision": spec["server"]["min_revision"],
            "client": {"kind": "plugin", "platform": "fixture", "name": "pi", "version": "0"},
        }
    )

    def _callback(event):
        event = (
            dict(sessionFile=str(file), turn="native-fixture-turn", openTools=0, pendingInput=0)
            | event
        )
        _send(
            {
                "type": "request",
                "id": "event",
                "operation": {
                    "kind": "session.boundary",
                    "moment": "event",
                    "delivery": os.environ["LF_CONTEXT_DELIVERY"],
                    "adapter": json.loads(os.environ["LF_PROVIDER_CONTEXT"]),
                    "callback": event,
                    "gap": None,
                    "printed": [],
                    "inbox": None,
                },
                "select": None,
                "sender": None,
                "request_id": None,
            }
        )

    _callback({"type": "session_start"})
    _callback({"type": "ui_prompt_start", "turn": None, "pendingInput": 1})
    _callback({"type": "ui_prompt_end", "turn": None})
    _record(args[-1] if "--" in args else "")
    _append("fixture-user", None, {"role": "user", "content": [{"type": "text", "text": args[-1]}]})
    _callback({"type": "agent_start"})
    events = [_message("Checking")]
    _append("fixture-step", "fixture-user", events[-1]["message"] | {"stopReason": "toolUse"})
    _callback(events[-1] | {"events": events})
    _callback(events[-1] | {"events": events})  # repeated observation is idempotent
    if os.environ.get("PI_FIXTURE_CRASH"):
        sys.exit(9)
    events.append(_message("native answer"))
    _append("fixture-answer", "fixture-step", events[-1]["message"])
    _callback(events[-1] | {"events": events})
    _callback({"type": "agent_settled", "aborted": False, "events": events})
    sys.exit(0)


for line in sys.stdin:
    request = json.loads(line)
    command = request["type"]
    response = {"type": "response", "command": command, "id": request.get("id"), "success": True}
    if command == "get_state":
        response["data"] = {"sessionFile": str(file), "sessionId": "fixture"}
    elif command == "prompt":
        text = request["message"]
        _record(text)
        if text == "reject":
            response.update(success=False, error="Model not found: fixture")
        else:
            response["data"] = {"disposition": "started"}
    _emit(response)
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
    if text == "dialog" or ASK.exists():
        _emit(
            {
                "type": "extension_ui_request",
                "method": "confirm",
                "id": "dialog",
                "title": "Use the fixture?",
            }
        )
        continue
    if text == "extension error":
        _emit({"type": "extension_error", "error": "fixture context delivery failed"})
        continue
    if text == "wait" or HOLD.exists():
        _emit({"type": "agent_start"})
        if HOLD.exists():
            HOLD.with_name("holding").write_text(str(file))
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
    _tool_turn()
    entries = [json.loads(line) for line in file.read_text().splitlines()]
    count = sum(entry.get("message", {}).get("role") == "user" for entry in entries) + 1
    answer = '{"path":"alpha"}' if "PI_ROUTE_SENTINEL" in text else f"answer {count}: {text}"
    parent = entries[-1].get("id") if entries[-1]["type"] == "message" else None
    user = {"role": "user", "content": [{"type": "text", "text": text}]}
    _append(f"rpc-user-{count}", parent, user)
    _append(f"rpc-answer-{count}", f"rpc-user-{count}", _message(answer)["message"])
    _emit(_message(answer))
    _emit({"type": "agent_end", "messages": [], "willRetry": False})
    _emit({"type": "agent_settled", "aborted": False})
