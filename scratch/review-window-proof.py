"""Create disposable provider history for the Watch retention review."""
from datetime import UTC, datetime
import json
from pathlib import Path
import tempfile

root = Path(tempfile.mkdtemp(prefix="loo293-window-review-"))
for number, provider in enumerate(["claude", "codex"], 1):
    run = f"run_{number:032x}"
    directory = root / "runs" / "00" / run
    directory.mkdir(parents=True)
    path = directory / "native.jsonl"
    manifest = dict(schema_version=1, run_id=run, parent_run_id=None,
                    created_at=datetime.now(UTC).isoformat(), harness=provider, model=None,
                    surface="tui", cwd="/tmp", repo=None, worktree=None, skill=None,
                    subjects=[dict(selector="task:LOO-293", source="declared")],
                    launch=None, context=None, runtime_path=None, runtime_digest=None,
                    host="review", boot_id=None)
    (directory / "manifest.json").write_text(json.dumps(manifest))
    (directory / "provider-session.json").write_text(json.dumps(dict(
        schema_version=1, provider_session_id="session", account_id=None,
        native_source=dict(kind="jsonl", path=str(path)))))
    rows = [] if provider == "claude" else [dict(type="session_meta", payload=dict(id="session"))]
    for ordinal in range(2305):
        text = f"{provider} record {ordinal}"
        if provider == "claude":
            row = dict(type="assistant", sessionId="session", uuid=f"record-{ordinal}",
                       message=dict(id=f"message-{ordinal}", content=[dict(type="text", text=text)]))
        else:
            row = dict(type="response_item", payload=dict(id=f"record-{ordinal}", type="message",
                       role="assistant", content=[dict(type="output_text", text=text)]))
        if ordinal < 2304:
            rows.append(row)
        else:
            (root / f"{provider}-arrival.jsonl").write_text(json.dumps(row) + "\n")
    path.write_text("".join(json.dumps(row) + "\n" for row in rows))
Path("/tmp/loo293-window-proof-root").write_text(str(root))
print(root)
