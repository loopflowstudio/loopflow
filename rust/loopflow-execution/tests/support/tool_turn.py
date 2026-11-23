"""A provider's side of the hooks lf declared, for a turn that runs one tool.

Every harness asks lf for context when its conversation starts and for news
after each tool, by running `lf __context-block` with a callback on stdin.
A fixture copies this script into LF_HOME; a stand-in that finds it there runs
it mid-turn with the hooks its harness was given, and the fixture reads, in
order:

- `hook-context`: what the start hook returned
- `holding`: the tool is running
- once the fixture writes `tool-done`, `hook-steer`: what the tool hook returned

Usage: tool_turn.py THREAD HOOKS_JSON. Claude and Codex pass the hook settings
lf declared (`{"SessionStart": [...], "PostToolUse": [...]}`). OpenCode and Pi,
whose plugins start lf themselves, pass `{"start": CALL, "steer": CALL}` with
CALL `{"argv": [...], "event": name, "without": [environment names],
"env": {...}, "cwd": path}`.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

home = Path(os.environ["LF_HOME"])
thread, hooks = sys.argv[1], json.loads(sys.argv[2])
if "SessionStart" in hooks:
    declared = lambda event: {"argv": hooks[event][0]["hooks"][0]["command"], "event": event}
    hooks = {"start": declared("SessionStart"), "steer": declared("PostToolUse")}


def ask(call):
    env = {k: v for k, v in os.environ.items() if k not in call.get("without", [])}
    callback = {"session_id": thread, "hook_event_name": call["event"], "source": "startup"}
    output = subprocess.run(
        call["argv"],
        shell=isinstance(call["argv"], str),
        input=json.dumps(callback),
        env=env | call.get("env", {}),
        cwd=call.get("cwd"),
        text=True,
        capture_output=True,
        timeout=60,
        check=True,
    ).stdout
    return json.loads(output)["hookSpecificOutput"]["additionalContext"] if output.strip() else ""


(home / "hook-context").write_text(ask(hooks["start"]))
(home / "holding").write_text(thread)
deadline = time.monotonic() + 120
while not (home / "tool-done").exists():
    if time.monotonic() >= deadline:
        raise TimeoutError("the fixture's tool was not released")
    time.sleep(0.02)
(home / "hook-steer").write_text(ask(hooks["steer"]))
