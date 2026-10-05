"""Render the compiled Clap catalog as the canonical command reference."""

import argparse
import json
from pathlib import Path


def _cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog", type=Path)
    parser.add_argument("--output", type=Path, default=Path("docs/lf-reference.md"))
    args = parser.parse_args()
    rows = json.loads(args.catalog.read_text())
    lines = [
        "# lf command reference",
        "",
        "```bash",
        "lf help --all",
        "lf help pr land",
        "lf monitor --json",
        "```",
        "",
        "Generated from the compiled Clap tree. Canonical command paths are shown; unique shortcuts also resolve. See",
        "[the workflow guide](lf.md) for examples. Hidden commands",
        "are internal process boundaries and are marked below.",
        "",
        "## Selection and output",
        "",
        "`--task` selects a Task checkout; `--wt` selects an existing worktree.",
        "`--wave` adds context without moving directories and must match a Task's",
        "owning Wave. Named direct Flows are independent contributions; `flow start`",
        "selects the Task's saved managed Flow. Saved state, identity and feedback",
        "survive continuation. `task restart` explicitly replaces that workflow.",
        "",
        "A preference (`--account`) permits fallback. A restriction (`--only-account`)",
        "limits this launch and its children. Saved Flows retain provider selections;",
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
        "lf --task EXP-12 flow start",
        "lf flow resume FLOW_ID --retry",
        "lf task restart EXP-12 --flow feature",
        "```",
        "",
        "Resume retains the captured graph. Retry retains its position. Restart",
        "deliberately captures a replacement. Completing a review returns feedback;",
        "the authored Flow decides what follows.",
        "",
    ]
    for row in rows:
        if row["kind"] != "command":
            continue
        path = row["path"]
        lines += ["## " + " ".join(path), "", row.get("about") or "Command namespace.", ""]
        if row["hidden"]:
            lines += ["Internal command; invoked by the owning operation.", ""]
        arguments = [item for item in rows if item["kind"] == "argument" and item["path"] == path]
        if arguments:
            lines += ["| Argument | What it does |", "|---|---|"]
            for item in arguments:
                label = (
                    "--" + item["long"]
                    if item["long"]
                    else ("-" + item["short"] if item["short"] else "<" + item["id"] + ">")
                )
                if item["short"] and item["long"]:
                    label += " / -" + item["short"]
                detail = item.get("help") or item["id"].replace("_", " ")
                if item["defaults"]:
                    detail += " Default: " + ", ".join(item["defaults"]) + "."
                if item["hidden"]:
                    detail += " Internal."
                lines.append(f"| `{_cell(label)}` | {_cell(detail)} |")
            lines.append("")
    args.output.write_text("\n".join(lines))


if __name__ == "__main__":
    main()
