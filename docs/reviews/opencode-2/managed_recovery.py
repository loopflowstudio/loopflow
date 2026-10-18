"""Observe managed restart semantics in disposable homes, without adopting them."""

import json
import os
import shutil
import signal
import subprocess
import tempfile
from pathlib import Path

from native_turns import Model, Native


def _scenario(crash: bool) -> dict:
    model = Model(hold=2)
    # A real native shell proves the launch environment and session location survived.
    environment_check = (
        'printf "%s\\n" "$LOO504_LAUNCH"; pwd; '
        'test -z "$OPENCODE_PASSWORD$OPENCODE_SERVER_PASSWORD" && printf lease-not-in-env'
    )
    model.tool_commands = {
        1: "printf x >> completed-tool; " + environment_check,
        3: environment_check + "; printf '\\n'; cat completed-tool",
    }
    with tempfile.TemporaryDirectory(prefix="loo504-managed-") as directory, model:
        root = Path(directory).resolve()
        native = Native(root, model, managed=True)
        native.server.environment["LOO504_LAUNCH"] = "launch-owned-probe"
        with native.server:
            first_pid = native.server.process.pid
            session = native.create()
            admission = native.prompt(session, "Initiate native tool turn")
            assert model.arrived.wait(20), "second model request did not arrive"
            native.wait_for_assistant(session, 2)
            before = native.history(session)
            assert len(model.requests) == 2
            assert not any(m["type"] == "idle" for m in before["messages"])
            if crash:
                # This is the deliberate crash input, not successful graceful cleanup.
                os.killpg(first_pid, signal.SIGKILL)
                native.server.process.wait(timeout=15)
            native.server.stop()
            model.release.set()
            native.server.start()
            assert native.server.process.pid != first_pid
            native.wait_for_assistant(session, 3)
            assert len(model.requests) >= 3, "managed startup did not recover model work"
            after = native.settle(session)
            assert len(model.requests) == 4
            assert after["info"]["id"] == session
            users = [m for m in after["messages"] if m["type"] == "user"]
            assert [m["id"] for m in users] == [admission["id"]], "original input was replayed"
            notices = [
                m
                for m in after["messages"]
                if m["type"] == "synthetic" and m.get("metadata", {}).get("notice") == "restart"
            ]
            assert len(notices) == 1, "restart did not emit exactly one synthetic notice"
            notice = notices[0]
            model_messages = model.requests[2]["messages"]
            assert any(
                m["role"] == "user" and notice["text"] in json.dumps(m["content"])
                for m in model_messages
            ), "synthetic notice was not model-visible user content"
            assistants = [m for m in after["messages"] if m["type"] == "assistant"]
            assert assistants[-1].get("error") is None
            assert any(p.get("text") == "answer-4" for p in assistants[-1]["content"])
            tools = [p for m in assistants for p in m["content"] if p["type"] == "tool"]
            expected_output = f"launch-owned-probe\n{native.repo}\nlease-not-in-env"
            assert len(tools) == 2
            assert tools[0]["state"]["content"] == [{"type": "text", "text": expected_output}], (
                tools[0]["state"]["content"]
            )
            assert tools[1]["state"]["content"] == [
                {"type": "text", "text": expected_output + "\nx"}
            ]
            assert (native.repo / "completed-tool").read_text() == "x"
            terminals = [m for m in after["messages"] if m["type"] == "idle"]
            assert len(terminals) == 1 and terminals[0]["outcome"] == "succeeded"
            assert native.history(session) == after
            native.server.stop()
            native.server.start()
            reread = native.settle(session)
            assert reread == after, "completed session changed on subsequent managed restart"
            assert len(model.requests) == 4
            result = {
                "native_session_id": session,
                "original_admission": admission,
                "restart_notice": notice,
                "model_requests": len(model.requests),
                "notice_is_model_visible_user_content": True,
                "launch_environment_and_session_cwd_preserved": True,
                "lease_absent_from_tool_environment": True,
                "tool_calls": len(tools),
                "original_tool_side_effect_count": 1,
                "completed_snapshot_unchanged_after_restart": True,
                "history": after,
                "recovered_model_messages": model_messages,
            }
            # Retain native evidence, removing only disposable path names.
            return json.loads(json.dumps(result).replace(str(root), "<disposable-root>"))


def main() -> None:
    assert os.environ.get("LOOPFLOW_NETWORK_ISOLATED"), "run through scripts/test_network.py"
    version = (
        subprocess.run(
            [shutil.which("opencode"), "--version"], capture_output=True, check=True, timeout=10
        )
        .stdout.decode()
        .strip()
    )
    print(
        json.dumps(
            {
                "provider_version": version,
                "scenarios": {"graceful": _scenario(False), "crash": _scenario(True)},
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
