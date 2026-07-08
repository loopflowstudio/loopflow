from __future__ import annotations

import subprocess
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
GOLDENS_DIR = ROOT / "tests" / "goldens"


def main() -> None:
    for case_path in sorted(GOLDENS_DIR.glob("*.yaml")):
        case = yaml.safe_load(case_path.read_text())
        cmd = [
            "cargo",
            "run",
            "-q",
            "-p",
            "loopflow",
            "--bin",
            "lf-prompt",
            "--",
            "--repo",
            str(ROOT / case["repo"]),
        ]
        if case.get("skill"):
            cmd.extend(["--skill", case["skill"]])
        if case.get("surface"):
            cmd.extend(["--surface", case["surface"]])
        if case.get("no_loopflow"):
            cmd.append("--no-loopflow")
        for direction in case.get("directions", []):
            cmd.extend(["--direction", direction])
        for docs_target in case.get("docs", []):
            cmd.extend(["--docs", docs_target])
        cmd.extend(["--diff-files", "true" if case["diff_files"] or case["diff"] else "false"])
        cmd.extend(["--diff", "true" if case["diff"] else "false"])
        cmd.extend(["--clipboard", "true" if case["clipboard"] else "false"])
        if case.get("wave"):
            cmd.extend(["--wave", case["wave"]])

        result = subprocess.run(cmd, check=True, capture_output=True, text=True, cwd=ROOT)
        case_path.with_suffix(".md").write_text(result.stdout)


if __name__ == "__main__":
    main()
