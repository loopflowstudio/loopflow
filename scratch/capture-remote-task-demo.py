"""Capture the existing remote Task scenarios with the worktree CLI."""

import difflib
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "rust/loopflow/tests/task_remote_tests.rs"
TARGET = ROOT / "rust/loopflow/tests/task_remote_demo_capture.rs"


def main() -> None:
    original = SOURCE.read_text()
    source = original
    replacements = [
        (
            '    let account_home = target.path().join(".lf/accounts/claude/fixture");',
            '    assert!(runtime.block_on(target_store.list_tasks(None)).unwrap().is_empty());\n'
            '    println!("BEFORE: target Tasks = 0; clone predates the implementation branch");\n'
            '    let account_home = target.path().join(".lf/accounts/claude/fixture");',
        ),
        (
            '    let command = |args: &[&str]| {\n        Command::new(env!("CARGO_BIN_EXE_lf"))',
            '    let command = |args: &[&str]| {\n        let output = Command::new(env!("CARGO_BIN_EXE_lf"))',
        ),
        (
            '            .unwrap()\n    };\n    let added = command',
            '            .unwrap();\n'
            '        println!("\\n$ {} {}\\nexit: {}\\nstdout:\\n{}\\nstderr:\\n{}",\n'
            '            env!("CARGO_BIN_EXE_lf"), args.join(" "), output.status,\n'
            '            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));\n'
            '        output\n    };\n    let added = command',
        ),
        (
            '        let output = invoke(selector, fixture.task.id.as_str());',
            '        let output = invoke(selector, "INF-123");',
        ),
        (
            '    // A later source commit must not reset a retained, dirty target checkout.',
            '    println!("AFTER TWO LAUNCHES: {}", json!({\n'
            '        "tasks": tasks.len(), "task_id": task.id, "source_legacy_id": fixture.task.id,\n'
            '        "checkout": task.worktree, "branch": branch, "head": repo.head_sha(),\n'
            '        "skill_observed": fs::read_to_string(target.path().join(".lf/seen-implementation")).unwrap(),\n'
            '        "skill_checkouts": seen, "prs": 1, "worktrees_including_main": 2\n'
            '    }));\n'
            '    // A later source commit must not reset a retained, dirty target checkout.',
        ),
        (
            '        "keep local work"\n    );\n}\n\n#[test]',
            '        "keep local work"\n    );\n'
            '    println!("PRESERVED AFTER REJECTION: HEAD = {}; local-notes = keep local work", retained_head);\n'
            '}\n\n#[test]',
        ),
        (
            '        assert_eq!(task.id, expected_id);',
            '        assert_eq!(task.id, expected_id);\n'
            '        println!("COLD MACHINE {}: Task {} at {}; observed_at = {}", name, task.id, head, task.plan.pm_snapshot_synced_at);',
        ),
        (
            '    let dirty = invoke();',
            '    let dirty = invoke();\n    println!("SOURCE UNCOMMITTED (transport operation): {dirty}");',
        ),
        (
            '    let missing = invoke();',
            '    let missing = invoke();\n    println!("SOURCE UNPUSHED (transport operation): {missing}");',
        ),
    ]
    for before, after in replacements:
        if source.count(before) != 1:
            raise RuntimeError(f"Expected one demo capture anchor: {before!r}")
        source = source.replace(before, after, 1)
    patch = "".join(difflib.unified_diff(
        original.splitlines(keepends=True), source.splitlines(keepends=True),
        fromfile="task_remote_tests.rs", tofile="task_remote_demo_capture.rs",
    ))
    (ROOT / "scratch/remote-task-demo-capture.patch").write_text(patch)
    environment = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    with TARGET.open("x") as capture:
        capture.write(source)
    try:
        build = subprocess.run(
            ["cargo", "test", "-p", "loopflow", "--test", TARGET.stem, "--no-run", "--message-format=json"],
            cwd=ROOT, env=environment, stdout=subprocess.PIPE, text=True, check=True,
        )
        artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
        executable = next(item["executable"] for item in artifacts
                          if item.get("executable") and item.get("target", {}).get("name") == TARGET.stem)
        command = [sys.executable, "scripts/test_network.py", "/usr/bin/env",
                   f"LF_BIN={ROOT / 'target/debug/lf'}", executable, "--nocapture", "--test-threads=1"]
        run = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, text=True)
        transcript = run.stdout + run.stderr
        (ROOT / "scratch/remote-task-demo-output.txt").write_text(transcript)
        print(transcript)
        run.check_returncode()
    finally:
        TARGET.unlink()


if __name__ == "__main__":
    main()
