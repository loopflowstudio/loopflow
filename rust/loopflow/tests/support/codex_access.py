#!/usr/bin/env python3
"""Disposable Codex credentials, live usage, and resumable thread fixture."""

import base64
import json
import os
import sys
import time
from pathlib import Path


def _send(value: dict) -> None:
    print(json.dumps(value), flush=True)


def main() -> None:
    if "--version" in sys.argv:
        print("fixture")
        return
    home = Path(os.environ["CODEX_HOME"])
    thread = os.environ.get("CODEX_FIXTURE_THREAD", "0199a213-81c0-7800-8aa1-bbab2a035a53")
    history = home / f"turns-{thread}.json"
    turns = json.loads(history.read_text()) if history.exists() else []
    auth = json.loads((home / "auth.json").read_text())
    token = auth["tokens"]["access_token"]
    claims = auth["tokens"]["id_token"].split(".")[1]
    email = json.loads(base64.urlsafe_b64decode(claims + "=" * (-len(claims) % 4)))["email"]
    if "check-history" in sys.argv:
        assert token in ("fixture", "rotated")
        print((home / "history").read_text())
        return
    for line in sys.stdin:
        request = json.loads(line)
        if "id" not in request:
            continue
        method = request["method"]
        with (home / "probe-events").open("a") as events:
            event = {
                "method": method,
                "pid": os.getpid(),
                "thread": thread,
                "home": str(home),
                "email": email,
            }
            events.write(json.dumps(event) + "\n")
        result = {}
        if method == "account/read":
            refreshed = home / "refresh-to"
            if request["params"].get("refreshToken") and refreshed.exists():
                refreshed.replace(home / "auth.json")
                token = json.loads((home / "auth.json").read_text())["tokens"]["access_token"]
            result = {"account": {"email": email, "planType": "plus"}}
        elif method == "account/rateLimits/read":
            replacement = home / "replace-on-limits"
            if replacement.exists():
                replacement.replace(home / "auth.json")
            if token == "timeout":
                time.sleep(60)
            if token in ("rejected", "unavailable", "forbidden", "server-error"):
                code, message = {
                    "rejected": (-32603, "failed to fetch usage: HTTP 401 Unauthorized"),
                    "unavailable": (429, "retry later"),
                    "forbidden": (403, "forbidden"),
                    "server-error": (503, "service unavailable"),
                }[token]
                _send({"id": request["id"], "error": {"code": code, "message": message}})
                continue
            if token == "malformed":
                _send({"id": request["id"], "result": None})
                continue
            result = {"rateLimits": {"primary": {"usedPercent": 12, "windowDurationMins": 300}}}
        elif method == "account/rateLimitResetCredit/consume":
            with (home / "spent").open("a") as spent:
                spent.write(request["params"]["idempotencyKey"] + "\n")
            result = {"outcome": "reset"}
        elif method in ("thread/start", "thread/resume"):
            assert token in ("fixture", "rotated")
            if method == "thread/resume":
                assert request["params"]["threadId"] == thread
                assert turns
            result = {"thread": {"id": thread, "turns": turns}}
        elif method == "thread/turns/list":
            assert request["params"]["threadId"] == thread
            result = {"data": turns, "nextCursor": None}
        elif method == "turn/start":
            assert request["params"]["threadId"] == thread
            assert token in ("fixture", "rotated")
            turn = {"id": f"turn-{len(turns)}", "status": "completed", "items": []}
            turns.append(turn)
            history.write_text(json.dumps(turns))
            result = {"turn": turn}
        _send({"id": request["id"], "result": result})
        if method == "turn/start":
            params = {"threadId": thread, "turnId": turn["id"]}
            _send({"method": "turn/started", "params": {"threadId": thread, "turn": {**turn, "status": "inProgress"}}})
            _send({"method": "item/completed", "params": {**params, "item": {"type": "agentMessage", "id": "message", "text": "RECOVERED", "phase": "final_answer"}}})
            _send({"method": "turn/completed", "params": {"threadId": thread, "turn": turn}})


if __name__ == "__main__":
    main()
