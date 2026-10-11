#!/usr/bin/python3
import json
import os
import sys
import time
from pathlib import Path


def wait_for_stop() -> None:
    Path(os.environ["FIXTURE_STARTED"]).touch()
    for _ in range(600):
        if Path(os.environ["FIXTURE_STOP"]).exists():
            return
        time.sleep(0.1)


if "--version" in sys.argv:
    print("fixture")
    sys.exit(0)
if "--input-format" not in sys.argv:
    wait_for_stop()
    sys.exit(0)
for line in sys.stdin:
    message = json.loads(line)
    message["session_id"] = "holding-thread"
    print(json.dumps(message), flush=True)
    print(json.dumps({"type": "system", "subtype": "init", "session_id": "holding-thread"}), flush=True)
    wait_for_stop()
    print(json.dumps({"type": "result", "uuid": message["uuid"], "session_id": "holding-thread", "subtype": "success", "is_error": False, "result": "done"}), flush=True)
