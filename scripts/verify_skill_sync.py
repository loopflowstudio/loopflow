#!/usr/bin/env python3
"""Verify loopflow step-to-Skill sync and optional live vendor invocation."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SENTINEL = "LOOPFLOW_SKILL_SENTINEL_20260620"


def run(cmd: list[str], cwd: Path, *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=check)


def lf_command(repo_root: Path) -> list[str]:
    env_bin = os.environ.get("LF_BIN")
    if env_bin:
        return [env_bin]
    return [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(repo_root / "Cargo.toml"),
        "-p",
        "loopflow",
        "--bin",
        "lf",
        "--",
    ]


def write_probe_step(repo: Path) -> None:
    step_dir = repo / ".lf" / "steps"
    step_dir.mkdir(parents=True)
    (step_dir / "lfprobe.md").write_text(
        "---\ninteractive: false\n---\n"
        "Print exactly the loopflow skill sync sentinel.\n\n"
        f"Output only `{SENTINEL}` and no extra text.\n",
        encoding="utf-8",
    )


def verify_synced_files(repo: Path) -> None:
    for path in [
        repo / ".claude" / "skills" / "lfprobe" / "SKILL.md",
        repo / ".agents" / "skills" / "lfprobe" / "SKILL.md",
    ]:
        content = path.read_text(encoding="utf-8")
        assert "loopflow: true" in content, path
        assert "description: Print exactly the loopflow skill sync sentinel." in content, path
        assert SENTINEL in content, path


def maybe_run_live(repo: Path, live: bool) -> None:
    if not live:
        print("live vendor probe skipped (pass --live to run claude/codex)")
        return

    probes = [
        ("claude", ["claude", "-p", "/lfprobe"]),
        ("codex", ["codex", "exec", "$lfprobe"]),
    ]
    for name, cmd in probes:
        if shutil.which(cmd[0]) is None:
            raise RuntimeError(f"{name} CLI not found")
        result = run(cmd, repo, check=False)
        output = result.stdout + result.stderr
        if result.returncode != 0 or SENTINEL not in output:
            raise RuntimeError(
                f"{name} probe failed with {result.returncode}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
            )
        print(f"{name}: {SENTINEL}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--live", action="store_true", help="run claude -p and codex exec")
    args = parser.parse_args()

    source_root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="lf-skill-sync-") as tmp:
        repo = Path(tmp)
        run(["git", "init", "-q"], repo)
        write_probe_step(repo)
        run(lf_command(source_root) + ["op", "sync-skills"], repo)
        verify_synced_files(repo)
        maybe_run_live(repo, args.live)
    print("skill sync verified")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
