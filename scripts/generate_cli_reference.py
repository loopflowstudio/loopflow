"""Render the compiled Clap catalog as the canonical command reference."""

import argparse
import json
from collections.abc import Iterator
from pathlib import Path


def _cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def _commands(definition: dict) -> Iterator[dict]:
    yield definition
    for command in definition["commands"]:
        yield from _commands(command)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog", type=Path)
    parser.add_argument("--output", type=Path, default=Path("docs/lf-reference.md"))
    args = parser.parse_args()
    rows = _commands(json.loads(args.catalog.read_text())["definition"])
    lines = [
        "# lf command reference",
        "",
        "```bash",
        "lf help --all",
        "lf help pr land",
        "lf monitor --json",
        "```",
        "",
        "Generated from the compiled Clap tree. Canonical command paths are shown; "
        "unique shortcuts also resolve. See",
        "[the workflow guide](lf.md) for examples. This reference lists public commands.",
        "",
        "## Selection and output",
        "",
        "`--machine <label-or-id>` runs the command remotely; "
        "`--repo NAME_OR_PATH` overrides its saved repository.",
        "`--forward-agent` requires `--machine`.",
        "`--task` selects a Task checkout; `--wt` selects an existing worktree.",
        "`--wave` adds context without moving directories and must match a Task's",
        "owning Wave. `task run` places a Task's worktree and then runs like",
        "`lf --task ISSUE flow FLOW`; every Flow naming a Task is equally its work.",
        "",
        "A preference (`--account`) permits fallback. A restriction (`--only-account`)",
        "limits this launch and its children. A Flow retains its provider selections;",
        "children check their destination's access before starting a provider.",
        "Account observations distinguish unavailable, expired and measured capacity.",
        "",
        "JSON readers emit one document; `monitor active --watch --json` emits NDJSON",
        "until stdin closes. Diagnostics go to stderr. Exit 0 means the requested",
        "operation succeeded; 1 denotes an operational failure, 2 a syntax or lookup",
        "failure, and 130 interruption. A successful auto-merge request is not a merge.",
        "",
        "## Flow decisions and recovery",
        "",
        "```bash",
        "lf task run EXP-12 pursue",
        "lf flow show FLOW_ID --processes --json",
        "lf task interrupt EXP-12",
        "```",
        "",
        "Each start is a new Flow: one lf process and the step processes it starts. Its ID",
        "is the Flow process's. `--state` selects `current` (the Flow process has no recorded",
        "exit), `completed` (it succeeded) or `stopped` (it exited before the last",
        "step). A stopped Flow's Processes are its history; nothing resumes it. Inspect",
        "them, then launch the work that remains.",
        "",
    ]
    for row in rows:
        lines += ["## " + row["command"], "", row.get("description") or "Command namespace.", ""]
        arguments = [
            item for item in row["arguments"]
            if row["command"] == "lf" or not item["global"]
        ]
        if arguments:
            lines += ["| Argument | What it does |", "|---|---|"]
            for item in arguments:
                label = (
                    "--" + item["long"]
                    if item["long"]
                    else ("-" + item["short"] if item["short"] else "<" + item["name"] + ">")
                )
                if item["short"] and item["long"]:
                    label += " / -" + item["short"]
                detail = item.get("description") or item["name"].replace("_", " ")
                if item["global"]:
                    detail += " Applies everywhere."
                if item["defaults"]:
                    detail += " Default: " + ", ".join(item["defaults"]) + "."
                lines.append(f"| `{_cell(label)}` | {_cell(detail)} |")
            lines.append("")
    args.output.write_text("\n".join(lines))


if __name__ == "__main__":
    main()
