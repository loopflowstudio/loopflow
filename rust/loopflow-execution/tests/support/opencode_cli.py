#!/usr/bin/env python3
"""Disposable OpenCode CLI for fixtures: `opencode run` turns and the bare terminal client."""

import json
import os
import sqlite3
import subprocess
import sys
import time
import uuid
from pathlib import Path

HOME = Path(__file__).resolve().parent.parent
SESSIONS = HOME / "native-sessions.json"
LOCATIONS = HOME / "native-locations.json"


def _write(path, value):
    """Whole or not at all: a Task's conversation and its worker launch at once."""
    partial = path.with_name(f"{path.name}.{os.getpid()}")
    partial.write_text(json.dumps(value))
    partial.replace(path)


def _save(sessions):
    _write(SESSIONS, sessions)
    _store(sessions)


def _store(sessions):
    """OpenCode's own store, where lf reads a conversation's answers."""
    data = os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share")
    home = Path(data) / "opencode"
    home.mkdir(parents=True, exist_ok=True)
    now = int(time.time() * 1000)
    with sqlite3.connect(home / "opencode.db") as store:
        store.executescript(
            "CREATE TABLE IF NOT EXISTS session_v2(id TEXT PRIMARY KEY, directory TEXT, time_updated INTEGER);"
            "CREATE TABLE IF NOT EXISTS session_message(id TEXT PRIMARY KEY, session_id TEXT,"
            " type TEXT, seq INTEGER, time_created INTEGER, time_updated INTEGER, data TEXT);"
        )
        for session, messages in sessions.items():
            store.execute(
                "INSERT OR REPLACE INTO session_v2 VALUES(?, ?, ?)",
                (session, locations.get(session), now),
            )
            store.execute("DELETE FROM session_message WHERE session_id=?", (session,))
            for seq, message in enumerate(messages):
                data = {k: v for k, v in message.items() if k not in ("id", "type")}
                created = message.get("time", {}).get("created", now)
                store.execute(
                    "INSERT INTO session_message VALUES(?, ?, ?, ?, ?, ?, ?)",
                    (message["id"], session, message["type"], seq, created, now, json.dumps(data)),
                )


def _record_launch():
    if evidence := os.environ.get("LF_TEST_SESSION_ENV"):
        Path(evidence).write_text(os.environ.get("OPENCODE_API_KEY", ""))
    if evidence := os.environ.get("LF_RESUME_PROOF"):
        Path(evidence).write_text(os.environ["LF_CAPTURE_KEY"])
        Path(evidence + ".caller").write_text(os.environ["LF_AGENT_CALLER"])
        Path(evidence + ".args").write_text("\n".join(sys.argv[1:]))
    (HOME / "received.json").write_text(
        json.dumps(
            {
                "args": sys.argv[1:],
                "cwd": os.getcwd(),
                "participant": os.environ.get("LF_USER_NAME"),
                "opencode": json.dumps(CONFIG),
                "server": CONFIG,
                "model": json.loads((HOME / "native-model.json").read_text())
                if (HOME / "native-model.json").exists()
                else None,
            }
        )
    )
    with (HOME / "launched").open("a") as output:
        output.write(os.environ["LF_CAPTURE_KEY"] + "\n")
    with (HOME / "declarations").open("a") as output:
        declaration = {"capture": os.environ["LF_CAPTURE_KEY"], "as": os.environ.get("LF_AS")}
        output.write(json.dumps(declaration) + "\n")
    tool = HOME / "tool-command.json"
    if tool.exists():
        command = json.loads(tool.read_text())
        tool.unlink()
        result = subprocess.run(
            command["argv"], cwd=command["cwd"], capture_output=True, timeout=60
        )
        (HOME / "tool-result.json").write_text(
            json.dumps({"code": result.returncode, "stderr": result.stderr.decode()})
        )


CONTRACT = "Return the final answer as the declared JSON value: "


def _contract(parts):
    """The answer schema a Graph process supplied in context, if any."""
    text = "".join(part.get("text", "") for part in parts)
    start = text.rfind(CONTRACT)
    if start < 0:
        return None
    return json.JSONDecoder().raw_decode(text[start + len(CONTRACT) :])[0]


def _line(kind, session, **part):
    print(json.dumps({"type": kind, "sessionID": session, "part": part}), flush=True)


def _fail(session, message, reason):
    message["error"] = {"name": "APIError", "data": {"message": reason}}
    sessions[session].append(
        {
            "id": "msg_" + uuid.uuid4().hex,
            "type": "idle",
            "outcome": "failed",
            "error": message["error"],
            "time": {"created": int(time.time() * 1000)},
        }
    )
    _save(sessions)
    print(
        json.dumps({"type": "error", "sessionID": session, "error": message["error"]}),
        flush=True,
    )
    raise SystemExit(1)


def _plugins():
    """Each lf plugin's launch spec and the delivery file it reads context for."""
    for plugin in CONFIG.get("plugins", []):
        source = (Path(plugin.removeprefix("file://")) / "index.js").read_text()
        constant = lambda name: json.JSONDecoder().raw_decode(source.split(f"const {name} = ", 1)[1])[0]
        yield constant("spec"), constant("delivery")


def _tool_turn(session):
    """The plugin's calls to lf around a tool, for a fixture that left `tool_turn.py`."""
    script = HOME / "tool_turn.py"
    if not script.exists():
        return
    for spec, delivery in _plugins():
        lf = {
            "env": {"LF_HOME": spec["delivery"]["home"]},
            "cwd": spec["delivery"]["repo"],
            "event": "OpenCodeMessagesTransform",
        }
        block = [spec["executable"], "__context-block", "--delivery", delivery, "--moment"]
        hooks = {
            # The block is rendered without the launch's capability.
            "start": lf | {"argv": [*block, "start"], "without": ["LF_PROVIDER_CONTEXT", "LF_BARE_STEER"]},
            "steer": lf | {"argv": [*block, "steer"]},
        }
        subprocess.run([sys.executable, str(script), session, json.dumps(hooks)], check=True)


def _run(session, text):
    """One `opencode run`: a turn that prints its parts and exits."""
    _record_launch()
    if evidence := os.environ.get("LF_TEST_REPLAY_EVIDENCE"):
        Path(evidence).write_text(
            json.dumps(
                {
                    "input": os.environ["LF_CAPTURE_KEY"],
                    "request": {"text": text},
                    "config": CONFIG,
                    "instructions": [Path(p).read_text() for p in CONFIG.get("instructions", [])],
                }
            )
        )
    # The native context hook carries structured response instructions.
    instructions = "".join(spec.get("instructions", "") for spec, _ in _plugins())
    prompt = [{"text": instructions}, {"text": text}]
    sessions[session].append({"id": "msg_" + uuid.uuid4().hex, "type": "user", "text": text})
    _save(sessions)
    assistant = "msg_" + uuid.uuid4().hex
    message = {
        "id": assistant,
        "type": "assistant",
        "time": {"created": int(time.time() * 1000)},
        "modelID": "fixture",
        "providerID": "opencode",
        "content": [],
    }
    sessions[session].append(message)
    _save(sessions)
    _line("step_start", session, id="prt_" + uuid.uuid4().hex, messageID=assistant, type="step-start")
    _tool_turn(session)
    # The turn is under way, as the real CLI's first line shows, and stays open.
    deadline = time.monotonic() + 30
    if (HOME / "hold-turn").exists():
        (HOME / "holding").write_text(session)
    while (HOME / "hold-turn").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("fixture turn was not released")
        time.sleep(0.01)
    if (HOME / "ask-question").exists():
        # `run --auto` cancels the question and lets the model answer anyway.
        _line(
            "tool_use",
            session,
            id="call_question",
            messageID=assistant,
            type="tool",
            tool="question",
            state={
                "status": "error",
                "input": {"questions": [{"question": "Use the fixture?", "options": []}]},
                "error": "This non-interactive run cannot ask the user questions,"
                " so the question was cancelled.",
            },
        )
    answer = "Fixture completed."
    # A retried turn carries no contract of its own; the conversation does.
    saved = HOME / f"contract-{session}.json"
    if schema := _contract(prompt):
        saved.write_text(json.dumps(schema))
    fail = HOME / "fail-once"
    transient = HOME / "transient-once"
    if transient.exists() and "Decide the fixture." in str(prompt):
        transient.unlink()
        _fail(session, message, "fixture status 502")
    elif fail.exists():
        fail.unlink()
        _fail(session, message, "fixture failure")
    elif (HOME / "decide-enabled").exists():
        schema = json.loads(saved.read_text()) if saved.exists() else None
        if schema:
            assert schema["additionalProperties"] is False
            repeats = HOME / "remaining-passes"
            remaining = int(repeats.read_text()) if repeats.exists() else 0
            if "decision" in schema["properties"]:
                decision = "iterate" if remaining else "advance"
                value = {"decision": decision, "summary": "Proof observed", "reason": None}
                blocked = HOME / "blocked-once"
                if blocked.exists():
                    blocked.unlink()
                    value = {
                        "decision": "blocked",
                        "summary": None,
                        "reason": "Release target is missing",
                    }
                invalid = HOME / "invalid-output-once"
                if invalid.exists() or (HOME / "invalid-output-always").exists():
                    invalid.unlink(missing_ok=True)
                    value = {"decision": "unknown"}
                elif remaining:
                    repeats.write_text(str(remaining - 1))
            else:
                value = {"path": schema["properties"]["path"]["enum"][0]}
            with (HOME / "decide.log").open("a") as log:
                log.write(json.dumps(value) + "\n")
            # A provider answers in prose; the Graph process finds the value in it.
            answer = "Decision:\n```json\n" + json.dumps(value) + "\n```"
        if (HOME / "disconnect-after-tool").exists():
            # The process dies after its tool ran and before it answered.
            (HOME / "tool-effect").write_text("completed")
            raise SystemExit(1)
    message.update(
        finish="stop",
        tokens={"input": 20, "output": 5, "reasoning": 0, "cache": {"read": 0, "write": 0}},
        cost=0,
    )
    message["time"]["completed"] = int(time.time() * 1000)
    message["content"] = [
        {
            "id": "part_" + assistant,
            "sessionID": session,
            "messageID": assistant,
            "type": "text",
            "text": answer,
        }
    ]
    sessions[session].append(
        {
            "id": "msg_" + uuid.uuid4().hex,
            "type": "idle",
            "outcome": "succeeded",
            "error": None,
            "time": {"created": int(time.time() * 1000)},
        }
    )
    _save(sessions)
    _line("text", session, id="part_" + assistant, messageID=assistant, type="text", text=answer)


def _flag(name):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else None


if "--version" in sys.argv:
    raise SystemExit(0)
CONFIG = json.loads(os.environ.get("OPENCODE_CONFIG_CONTENT", "{}"))
sessions = json.loads(SESSIONS.read_text()) if SESSIONS.exists() else {}
locations = json.loads(LOCATIONS.read_text()) if LOCATIONS.exists() else {}
# `--session` names the conversation to continue, or to create.
session = _flag("--session") or "ses_" + uuid.uuid4().hex
assert session.startswith("ses"), session
sessions.setdefault(session, [])
locations[session] = os.getcwd()
_write(LOCATIONS, locations)
if model := CONFIG.get("model"):
    provider, _, name = model.partition("/")
    _write(HOME / "native-model.json", {"providerID": provider, "id": name})
_save(sessions)
if "run" not in sys.argv:
    _record_launch()
    if __WAIT__:  # noqa: F821 — substituted by the Rust fixture before execution
        sys.stdin.readline()
    raise SystemExit(0)
_run(session, sys.stdin.read())
