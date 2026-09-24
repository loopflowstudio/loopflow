"""Observe an owned native client whose launcher predates Run bindings."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


lf = Path("target/debug/lf").resolve()
expect_discovery = "--expect-discovery" in sys.argv
receipt_path = Path(__file__).with_name(
    "preexisting-client-after.json" if expect_discovery else "preexisting-client.json"
)
with tempfile.TemporaryDirectory(prefix="loo291-review19-") as temporary:
    home = Path(temporary)
    env = dict(os.environ)
    for key in ["LF_RUN_ID", "LF_RUN_DIR", "LF_WAVE_ID", "LF_TRACE_ID",
                "LF_PROCESS_ID", "LF_TERMINAL_ID", "LF_TERMINAL_TTY",
                "LF_HUMAN_SESSION", "LF_HUMAN_SESSION_RUN"]:
        env.pop(key, None)
    env.update(LF_HOME=str(home), LF_CONTROL_HOME=str(home), LF_BIN=str(lf),
               LF_DB_PATH=str(home / "loopflow.db"),
               LF_CONTROL_DB_PATH=str(home / "loopflow.db"))

    def query(*args: str):
        output = subprocess.run([str(lf), *args], env=env, capture_output=True,
                                text=True, check=True, timeout=10)
        return json.loads(output.stdout)

    session_id = "ask_preexisting-client"
    records = home / "human-sessions"
    records.mkdir()
    (records / f"{session_id}.json").write_text(json.dumps({
        "id": session_id, "parent_run_id": "run_00000000000000000000000000000002",
        "parent_run_dir": str(home / "parent"), "work": None, "work_selector": None,
        "title": "Owned local proof", "detail": "proof", "prompt": "local only",
        "cwd": str(home), "model": "opencode", "session_run_id": None,
        "ready_summary": None, "status": "waiting",
    }))
    prepared = query("session", "open", session_id, "--json")
    run_id = prepared["run_id"]
    directory = home / "runs" / run_id[4:6] / run_id
    (directory / "provider-session.json").write_text(json.dumps({
        "schema_version": 1, "provider_session_id": "ses_local-proof",
        "account_id": None,
    }))
    bin_path = home / "bin"
    bin_path.mkdir()
    provider = bin_path / "opencode"
    provider.write_text('#!/bin/sh\nif [ "$1" = --version ]; then exit 0; fi\n'
                        'printf "%s" "$LF_RUN_ID" > "$LF_PROOF_READY"\nread -r answer\n')
    provider.chmod(0o755)
    ready = home / "ready"
    env.update(PATH=f"{bin_path}:{env['PATH']}", LF_PROOF_READY=str(ready))
    child = subprocess.Popen([str(lf), "session", "open", session_id], env=env,
                             stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                             stderr=subprocess.PIPE, text=True)
    removed = {}
    try:
        deadline = time.monotonic() + 10
        while not ready.exists() and time.monotonic() < deadline and child.poll() is None:
            time.sleep(0.02)
        assert ready.exists(), "owned provider did not start"
        session_before = query("session", "open", session_id, "--json")
        before = query("runs", "--active", "--json")
        assert session_before["state"] == "active"
        assert any(run["id"] == run_id for run in before["runs"]), before
        # Model an older launcher: keep the manifest, history and live provider
        # receipt, but omit the newly introduced discovery index.
        for marker in (home / "run-bindings").glob("*.json"):
            removed[marker] = marker.read_bytes()
            marker.unlink()
        if not expect_discovery:
            assert removed
        session_after = query("session", "open", session_id, "--json")
        without_bindings = query("runs", "--active", "--json")
        receipt = {"run_id": run_id, "session_before": session_before["state"],
                   "session_after": session_after["state"], "before": before,
                   "without_bindings": without_bindings,
                   "provider_still_running": child.poll() is None,
                   "binding_count_removed": len(removed)}
        receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
        print(json.dumps(receipt, indent=2))
        if expect_discovery:
            assert session_after["state"] == "active"
            assert any(run["id"] == run_id for run in without_bindings["runs"])
            assert without_bindings["gaps"] == []
    finally:
        for marker, content in removed.items():
            marker.write_bytes(content)
        _, stderr = child.communicate("done\n", timeout=10)
        assert child.returncode == 0, stderr

    after_exit = query("runs", "--active", "--json")
    assert after_exit["runs"] == [] and after_exit["gaps"] == [], after_exit
    (directory / "provider-clients" / "broken.json").write_text("{")
    failed_read = query("runs", "--active", "--json")
    assert failed_read["runs"] == [] and failed_read["gaps"], failed_read
    text_read = subprocess.run([str(lf), "runs", "--active"], env=env,
                               capture_output=True, text=True, check=True, timeout=10)
    assert "Unavailable:" in text_read.stdout and "No active Runs." not in text_read.stdout
    receipt.update(after_exit=after_exit, corrupt_receipt=failed_read,
                   corrupt_receipt_text=text_read.stdout)
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print("Exit removes active Run; unreadable ownership remains explicitly unavailable.")
