#!/usr/bin/env python3
"""Manually check a real first response through the terminal's provider wrappers."""

import argparse
import os
import secrets
import shlex
import shutil
import subprocess
import tempfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", choices=["claude", "codex"], required=True)
    parser.add_argument("--plan", action="store_true", help="Use Claude's native plan mode")
    parser.add_argument(
        "--prepare-only", action="store_true", help="Print the launch without calling a provider"
    )
    args = parser.parse_args()
    if args.plan and args.agent != "claude":
        parser.error("--plan applies to Claude")
    source = Path(__file__).resolve().parents[1]
    binary = source / "target/debug/lf"
    if not binary.is_file():
        parser.error("Build the candidate first: cargo build -p loopflow --bin lf")
    root = Path(tempfile.mkdtemp(prefix="lf-context-check-"))
    repo = root / "repo"
    repo.mkdir()
    subprocess.run(["git", "init", "-q", "--initial-branch=main", str(repo)], check=True)
    subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "-c",
            "user.name=Context Check",
            "-c",
            "user.email=context-check@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "Context check",
        ],
        check=True,
    )
    wave = repo / "wave/infrastructure"
    wave.mkdir(parents=True)
    for name in ["GOAL.md", "MEMORY.md"]:
        shutil.copyfile(source / "wave/infrastructure" / name, wave / name)
    config = repo / ".lf"
    config.mkdir()
    (config / "config.yaml").write_text(
        "diff: false\ndiff_files: false\npaste: false\ndocs: [wave/infrastructure, probe.md]\n"
    )
    marker = secrets.token_hex(12)
    (repo / "probe.md").write_text(
        f"Context marker: {marker}\nOpenCode, opencode and openclaw are names in this reference.\n"
        + "Preserve infrastructure configurations and implementation.\n" * 1_400
    )
    if args.plan:
        settings = repo / ".claude"
        settings.mkdir()
        (settings / "settings.json").write_text('{"permissions":{"defaultMode":"plan"}}\n')
    environment = {
        key: value for key, value in os.environ.items() if not key.startswith(("LF_", "LOOPFLOW_"))
    }
    environment.update(LF_HOME=str(root / "machine"), LF_BIN=str(binary))
    command = [
        str(binary),
        "-i",
        "-a",
        args.agent,
        ":",
        "Without tools, reply with the context marker from probe.md, one provider name "
        "from that file, and the heading of the first Infrastructure memory entry. Stop there.",
    ]
    print(f"Expected marker: {marker}", flush=True)
    print(f"Retained check directory: {root}", flush=True)
    print(
        "Record the first response, native plan mode (if selected), and host/provider titles.",
        flush=True,
    )
    if args.prepare_only:
        print(f"cd {shlex.quote(str(repo))}")
        cleared = [
            item
            for key in os.environ
            if key.startswith(("LF_", "LOOPFLOW_"))
            for item in ("-u", key)
        ]
        print(
            shlex.join(
                ["env", *cleared, f"LF_HOME={root / 'machine'}", f"LF_BIN={binary}", *command]
            )
        )
        return 0
    # Preserve HOME and PATH deliberately: this manual check uses the caller's
    # real provider account and cmux wrappers, but a disposable Loopflow Machine.
    return subprocess.call(command, cwd=repo, env=environment)


if __name__ == "__main__":
    raise SystemExit(main())
