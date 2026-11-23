"""Append a completed turn to the rollout Codex keeps for a thread, where lf
reads final answers and usage: `<CODEX_HOME>/sessions/YYYY/MM/DD/rollout-<time>-<thread>.jsonl`.

Usage: codex_rollout.py THREAD TURN ANSWER_JSON
"""

import glob
import json
import os
import sys
import time

thread, turn, answer = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
home = os.environ.get("CODEX_HOME") or os.path.expanduser("~/.codex")
found = glob.glob(os.path.join(home, "sessions", "*", "*", "*", f"rollout-*-{thread}.jsonl"))
now = time.gmtime()
stamp = time.strftime("%Y-%m-%dT%H:%M:%S.000Z", now)
rows = []
if found:
    path = found[0]
else:
    day = os.path.join(home, "sessions", time.strftime("%Y/%m/%d", now))
    os.makedirs(day, exist_ok=True)
    path = os.path.join(day, f"rollout-{time.strftime('%Y-%m-%dT%H-%M-%S', now)}-{thread}.jsonl")
    rows.append({"type": "session_meta", "payload": {"id": thread, "cwd": os.getcwd()}})
# Codex reports the conversation's running total with each turn's own count.
turns = 1
if found:
    with open(path) as rollout:
        turns += sum(json.loads(row)["payload"].get("type") == "task_complete" for row in rollout)
count = lambda turns: {"input_tokens": 20 * turns, "cached_input_tokens": 0, "output_tokens": 5 * turns}
rows += [
    {"type": "event_msg", "payload": {"type": "task_started", "turn_id": turn}},
    {"type": "event_msg",
     "payload": {"type": "token_count",
                 "info": {"total_token_usage": count(turns), "last_token_usage": count(1)}}},
    {"type": "event_msg",
     "payload": {"type": "task_complete", "turn_id": turn, "last_agent_message": answer}},
]
with open(path, "a") as rollout:
    for row in rows:
        rollout.write(json.dumps({"timestamp": stamp} | row) + "\n")
