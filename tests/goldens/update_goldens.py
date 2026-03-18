from __future__ import annotations

import subprocess
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
GOLDENS_DIR = ROOT / "tests" / "goldens"


def _bool_flag(name: str, value: bool) -> list[str]:
    return [f"--{name}", "true" if value else "false"]


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
        if case.get("step"):
            cmd.extend(["--step", case["step"]])
        if case.get("surface"):
            cmd.extend(["--surface", case["surface"]])
        for direction in case.get("directions", []):
            cmd.extend(["--direction", direction])
        cmd.extend(_bool_flag("lfdocs", case["lfdocs"]))
        cmd.extend(_bool_flag("diff-files", case["diff_files"] or case["diff"]))
        cmd.extend(_bool_flag("diff", case["diff"]))
        cmd.extend(_bool_flag("clipboard", case["clipboard"]))
        if case.get("area"):
            cmd.extend(["--area", case["area"]])
        if case.get("wave"):
            cmd.extend(["--wave", case["wave"]])

        result = subprocess.run(cmd, check=True, capture_output=True, text=True, cwd=ROOT)
        case_path.with_suffix(".md").write_text(result.stdout)


if __name__ == "__main__":
    main()
