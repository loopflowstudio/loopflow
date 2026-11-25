#!/usr/bin/env python3
"""List the store members marked leaving, and start the removals that are due.

    uv run python scripts/due_removals.py            # every marked Task and its drop date
    uv run python scripts/due_removals.py --start    # run each due, unstarted removal Task
    uv run python scripts/due_removals.py install    # weekly LaunchAgent, Monday 09:00
    uv run python scripts/due_removals.py uninstall

A migration marks what the product stopped using with `-- leaving: <member>
<Task ID>`. A Task is due once its marking migration has been on origin/main for
`STAGING_DAYS`. A member a contract already dropped, or a later migration kept
with `-- keeping: <member> <Task ID>`, is no longer listed.
`--start` runs `lf task run <ID> -a claude` only for Tasks `lf task status`
shows `unstarted`, so running it again never restarts work.

Stdlib only; the LaunchAgent runs it through `uv run`.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import plistlib
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_migrations  # noqa: E402

LABEL = "com.loopflow.due-removals"
AGENT = Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist"
LOG = Path.home() / "Library/Logs/loopflow-due-removals.log"


@dataclass
class Removal:
    task: str
    members: list[str] = field(default_factory=list)
    # The latest drop date among its marks; None while a marking migration is unmerged.
    drop_after: dt.date | None = None
    merged: bool = True

    def due(self, today: dt.date) -> bool:
        return self.merged and self.drop_after is not None and self.drop_after <= today


def removals(local: dict[str, bytes], added: dict[str, dt.datetime]) -> list[Removal]:
    """Every Task with a mark on a member that still exists, ordered by Task."""
    catalog = check_migrations.Catalog(local, added)
    schema = check_migrations._schema(catalog.replay())
    by_task: dict[str, Removal] = {}
    for member, (name, mark) in check_migrations.active_leaving_marks(catalog).items():
        if not schema.has(member):
            continue
        removal = by_task.setdefault(mark.task, Removal(mark.task))
        removal.members.append(member)
        if name not in added:
            removal.merged = False
            continue
        date = check_migrations.drop_after(added[name])
        removal.drop_after = max(filter(None, (removal.drop_after, date)))
    return sorted(by_task.values(), key=lambda removal: removal.task)


def _task_state(task: str) -> str:
    result = subprocess.run(
        ["lf", "task", "status", task, "--json"],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return "unknown"
    try:
        return json.loads(result.stdout)["task"]["state"]
    except (json.JSONDecodeError, KeyError, TypeError):
        return "unknown"


def _describe(removal: Removal) -> str:
    members = ", ".join(removal.members[:2]) + (", …" if len(removal.members) > 2 else "")
    if not removal.merged:
        return f"{members}  not merged"
    return f"{members}  drop after {removal.drop_after:%Y-%m-%d}"


def _list(start: bool) -> None:
    local = check_migrations._local_migrations()
    main = check_migrations._migrations_at("origin/main")
    added = check_migrations.Catalog.of_origin_main(main).added if main is not None else {}
    today = dt.date.today()
    for removal in removals(local, added):
        state = _task_state(removal.task)
        print(f"{removal.task}  {_describe(removal)}  {state}")
        if start and state == "unstarted" and removal.due(today):
            print(f"starting {removal.task}", flush=True)
            subprocess.run(
                ["lf", "task", "run", removal.task, "-a", "claude"],
                stdin=subprocess.DEVNULL,
                check=False,
            )


def _install() -> None:
    script = Path(__file__).resolve()
    uv = shutil.which("uv")
    if uv is None:
        raise SystemExit("uv is not on PATH; install it first")
    AGENT.parent.mkdir(parents=True, exist_ok=True)
    LOG.parent.mkdir(parents=True, exist_ok=True)
    plist = {
        "Label": LABEL,
        # The replay needs uv's managed Python: macOS's system SQLite rejects
        # released migrations.
        "ProgramArguments": [uv, "run", "python", str(script), "--start"],
        "WorkingDirectory": str(script.parent.parent),
        # launchd starts with a bare PATH; keep the one that finds `lf` and `git` now.
        "EnvironmentVariables": {"PATH": os.environ.get("PATH", "/usr/bin:/bin")},
        "StartCalendarInterval": {"Weekday": 1, "Hour": 9, "Minute": 0},
        "StandardOutPath": str(LOG),
        "StandardErrorPath": str(LOG),
    }
    AGENT.write_bytes(plistlib.dumps(plist))
    target = f"gui/{os.getuid()}"
    subprocess.run(["launchctl", "bootout", f"{target}/{LABEL}"], capture_output=True)
    subprocess.run(["launchctl", "bootstrap", target, str(AGENT)], check=True)
    print(f"installed {AGENT}; output goes to {LOG}")


def _uninstall() -> None:
    subprocess.run(["launchctl", "bootout", f"gui/{os.getuid()}/{LABEL}"], capture_output=True)
    AGENT.unlink(missing_ok=True)
    print(f"removed {AGENT}")


def main() -> None:
    parser = argparse.ArgumentParser(description="List leaving store members and their removals.")
    parser.add_argument("action", nargs="?", choices=("install", "uninstall"))
    parser.add_argument("--start", action="store_true", help="run each due, unstarted Task")
    args = parser.parse_args()
    if args.action == "install":
        _install()
    elif args.action == "uninstall":
        _uninstall()
    else:
        _list(args.start)


if __name__ == "__main__":
    main()
