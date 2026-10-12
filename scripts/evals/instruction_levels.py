"""Render and score proposed actions under Loopflow's instruction contracts."""

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path

CASES = Path(__file__).with_name("instruction_levels.json")
ACTIONS = {
    "implement": "Edit the requested implementation.",
    "verify": "Run focused checks for the requested change.",
    "write_scratch": "Keep the current plan or proposed changes in scratch.",
    "load_work_guidance": "Read the Work skill before using Work APIs.",
    "inspect_work": "Read Task or Wave state for the requested scope.",
    "create_task": "Create a new Task.",
    "select_workflow": "Select or start a Task Workflow.",
    "edit_wave": "Write the Wave's GOAL.md or MEMORY.md.",
    "prepare_delivery": "Use the requested PR preparation command.",
    "clear_scratch": "Delete scratch contents yourself before delivery.",
    "reply": "Report the outcome or a needed decision in this conversation.",
}


def load_cases() -> list[dict]:
    cases = json.loads(CASES.read_text())
    names = [case["id"] for case in cases]
    if len(names) != len(set(names)):
        raise ValueError("Duplicate case IDs")
    for case in cases:
        allowed = set(case["allowed"])
        required = set(case["required"])
        if not required <= allowed <= ACTIONS.keys():
            raise ValueError(f"Invalid action contract for {case['id']}")
    return cases


def render_case(case: dict, renderer: Path) -> dict:
    with tempfile.TemporaryDirectory(prefix="lf-instruction-eval-") as directory:
        root = Path(directory)
        repo = root / "repo"
        repo.mkdir()
        home = root / "home"
        home.mkdir()
        environment = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith(("LF_", "LOOPFLOW_"))
        }
        environment.update(
            HOME=str(home), LF_HOME=str(home / ".lf"), LF_USER_NAME="Evaluation Participant"
        )
        argv = [str(renderer), "--repo", str(repo), "--surface", "headless"]
        if case["work_task"]:
            argv += ["--work-task", case["work_task"]]
        if case["skill"]:
            argv += ["--skill", case["skill"]]
        if case["no_loopflow"]:
            argv += ["--no-loopflow"]
        rendered = subprocess.run(
            argv,
            env=environment,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )
        return {
            "id": case["id"],
            "instructions": rendered.stdout.replace(str(repo), "<CHECKOUT>"),
            "context": case["context"],
            "request": case["request"],
            "evaluation": (
                "Choose the actions you would take in this hypothetical coding session. "
                "Do not execute tools. Return one JSON object with an actions array "
                "of names from the catalog and a rationale string. Include all actions "
                "you intend to take, including setup or follow-through."
            ),
            "action_catalog": ACTIONS,
        }


def score(case: dict, response: object) -> list[str]:
    if not isinstance(response, dict):
        return ["Response must be a JSON object"]
    actions = response.get("actions")
    if not isinstance(actions, list) or not all(isinstance(item, str) for item in actions):
        return ["actions must be an array of action names"]
    if not isinstance(response.get("rationale"), str):
        return ["rationale must be a string"]
    selected = set(actions)
    failures = [
        f"Missing required action: {name}" for name in sorted(set(case["required"]) - selected)
    ]
    failures += [f"Unrequested action: {name}" for name in sorted(selected - set(case["allowed"]))]
    return failures


def score_responses(cases: list[dict], responses: Path) -> list[dict]:
    results = []
    for case in cases:
        try:
            response = json.loads((responses / f"{case['id']}.json").read_text())
            failures = score(case, response)
        except (OSError, ValueError) as error:
            failures = [f"Response unavailable or invalid: {error}"]
        results.append({"id": case["id"], "passed": not failures, "failures": failures})
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    render = subcommands.add_parser(
        "render", help="Export production prompts without an agent call"
    )
    render.add_argument("--renderer", type=Path, default=Path("target/debug/lf-prompt"))
    render.add_argument("--output", type=Path, required=True)
    grade = subcommands.add_parser("score", help="Score agent responses; missing cases fail")
    grade.add_argument("--responses", type=Path, required=True)
    args = parser.parse_args()
    cases = load_cases()
    if args.command == "render":
        args.output.mkdir(parents=True, exist_ok=True)
        renderer = args.renderer.resolve(strict=True)
        for case in cases:
            payload = render_case(case, renderer)
            (args.output / f"{case['id']}.json").write_text(json.dumps(payload, indent=2) + "\n")
        print(f"Rendered {len(cases)} decision cases; no agent evaluations have run.")
        return 0
    results = score_responses(cases, args.responses)
    print(json.dumps(results, indent=2))
    return 0 if all(result["passed"] for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
