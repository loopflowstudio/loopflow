"""Exercise synthetic native records through the CLI against a read-only Task registry."""

import json
import os
from pathlib import Path
import subprocess
import tempfile


def _main() -> None:
    binary = Path(__file__).resolve().parents[1] / "target/debug/lf"
    home = Path(os.environ.get("LF_CONTROL_HOME") or os.environ.get("LF_HOME") or Path.home() / ".lf")
    database = Path(os.environ.get("LF_CONTROL_DB_PATH") or os.environ.get("LF_DB_PATH") or home / "loopflow.db")
    if not database.is_absolute():
        database = home / database
    with tempfile.TemporaryDirectory(prefix="lf-review-native-") as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        env["LF_CONTROL_HOME"] = str(root)
        env["LF_CONTROL_DB_PATH"] = str(database)
        for provider, digit in [("claude", "1"), ("codex", "2")]:
            run_id = "run_" + digit * 32
            directory = root / "runs" / (digit * 2) / run_id
            directory.mkdir(parents=True)
            manifest = dict(
                schema_version=1, run_id=run_id, parent_run_id=None,
                created_at="2026-09-23T00:00:00Z", harness=provider, model=None,
                surface="tui", cwd=temporary, repo=None, worktree=None, skill=None,
                subjects=[dict(selector="task:LOO-293", source="declared")],
                launch=None, context=None, runtime_path=None, runtime_digest=None,
                host="synthetic-review", boot_id=None,
            )
            (directory / "manifest.json").write_text(json.dumps(manifest))
            transcript = directory / "native.jsonl"
            receipt = dict(schema_version=1, provider_session_id="session", account_id=None,
                           native_source=dict(kind="jsonl", path=str(transcript)))
            (directory / "provider-session.json").write_text(json.dumps(receipt))
            if provider == "claude":
                records = [
                    dict(type="assistant", sessionId="session", uuid="assistant-1", message=dict(content=[
                        dict(type="tool_use", id="call-one", name="shell", input=dict(command="one")),
                        dict(type="tool_use", id="call-two", name="shell", input=dict(command="two")),
                    ])),
                    dict(type="user", sessionId="session", uuid="user-1", message=dict(content=[
                        dict(type="tool_result", tool_use_id="call-two", content="second result"),
                        dict(type="tool_result", tool_use_id="call-one", content="first result"),
                    ])),
                ]
            else:
                records = [dict(type="session_meta", payload=dict(id="session"))]
                records += [dict(type="response_item", payload=dict(type="function_call", call_id=call,
                            name="shell", arguments=command)) for call, command in [("call-one", "one"), ("call-two", "two")]]
                records += [dict(type="response_item", payload=dict(type="function_call_output", call_id=call,
                            output=result)) for call, result in [("call-two", "second result"), ("call-one", "first result")]]
            transcript.write_text("".join(json.dumps(record) + "\n" for record in records))

        def read() -> subprocess.CompletedProcess[str]:
            return subprocess.run([str(binary), "task", "output", "LOO-293", "--json"],
                                  env=env, cwd="/tmp", capture_output=True, text=True, timeout=30)

        result = read()
        result.check_returncode()
        for source in json.loads(result.stdout)["sources"]:
            items = [record["event"]["item"] for record in source["records"]]
            calls = [item for item in items if item["status"] == "running"]
            results = [item for item in items if item["status"] == "completed"]
            assert [item["id"] for item in calls] == ["call-one", "call-two"]
            assert [item["id"] for item in results] == ["call-two", "call-one"]
            print(json.dumps(dict(provider=source["provider"], synthetic_records=len(items),
                                  call_ids_preserved=all(any(call in json.dumps(item) for item in calls)
                                                         for call in ["call-one", "call-two"]),
                                  gaps=source["gaps"])))

        unrelated = root / "runs/33" / ("run_" + "3" * 32)
        unrelated.mkdir(parents=True)
        (unrelated / "manifest.json").write_text("{broken")
        result = read()
        result.check_returncode()
        page = json.loads(result.stdout)
        assert page["gaps"][0]["code"] == "discovery_incomplete"
        assert sum(len(source["records"]) for source in page["sources"]) == 8
        print(json.dumps(dict(unrelated_corrupt_manifest_returncode=result.returncode,
                              healthy_output_returned=bool(result.stdout.strip()),
                              gap_codes=[gap["code"] for gap in page["gaps"]],
                              error=result.stderr.strip())))
        watch = subprocess.run([str(binary), "task", "watch", "LOO-293", "--json"],
                               env=env, cwd="/tmp", capture_output=True, text=True, timeout=30)
        watch.check_returncode()
        snapshot = json.loads(watch.stdout)
        assert len(snapshot["runs"]) >= 2
        assert any(gap["code"] == "discovery_incomplete" for gap in snapshot["gaps"])
        print(json.dumps(dict(watch_runs=len(snapshot["runs"]),
                              watch_gap_codes=[gap["code"] for gap in snapshot["gaps"]])))


if __name__ == "__main__":
    _main()
