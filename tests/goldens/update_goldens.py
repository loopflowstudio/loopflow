from __future__ import annotations

from pathlib import Path

import yaml

from loopflow.lf.context import (
    ContextConfig,
    DiffMode,
    FilesetConfig,
    format_prompt,
    gather_prompt_components,
)
from loopflow.lf.directions import resolve_directions


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _load_cases(goldens_dir: Path) -> list[Path]:
    cases = sorted(path for path in goldens_dir.glob("*.yaml") if path.is_file())
    return cases


def _render_prompt(case: dict, repo_root: Path) -> str:
    directions = case.get("directions") or []
    direction_objs = resolve_directions(repo_root, directions) if directions else None

    diff = case.get("diff", False)
    diff_files = case.get("diff_files", False)
    if diff:
        diff_mode = DiffMode.DIFF
    elif diff_files:
        diff_mode = DiffMode.FILES
    else:
        diff_mode = DiffMode.NONE

    context_config = ContextConfig(
        diff_mode=diff_mode,
        files=FilesetConfig(paths=[], exclude=[], parent_docs=False),
        area=case.get("area"),
        wave=case.get("wave"),
        include_loopflow_doc=False,
        clipboard=case.get("clipboard", False),
        budget_area=0,
        budget_docs=0,
        budget_diff=0,
    )

    components = gather_prompt_components(
        repo_root=repo_root,
        step=case.get("step"),
        inline=None,
        step_args=None,
        run_mode=case.get("run_mode"),
        direction=direction_objs,
        context_config=context_config,
        config=None,
    )

    return format_prompt(components)


def main() -> None:
    root = _repo_root()
    goldens_dir = root / "tests" / "goldens"
    cases = _load_cases(goldens_dir)

    for case_path in cases:
        case = yaml.safe_load(case_path.read_text())
        repo_root = root / case["repo"]
        prompt = _render_prompt(case, repo_root)
        output_path = case_path.with_suffix(".md")
        output_path.write_text(prompt)
        print(f"Wrote {output_path}")


if __name__ == "__main__":
    main()
