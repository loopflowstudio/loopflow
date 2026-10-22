import pytest

from scripts.macos_inputs import affects_macos


@pytest.mark.parametrize(
    "paths",
    [
        ["docs/architecture.md", "website/index.html"],
        ["README.md", "AGENTS.md", "scratch/plan.md"],
        ["python/loopflow/cli.py", "python/tests/test_cli.py"],
        [".lf/wave/infrastructure/MEMORY.md"],
    ],
)
def test_documentation_website_and_python_changes_skip_macos(paths: list[str]) -> None:
    assert not affects_macos(paths)


@pytest.mark.parametrize(
    "path",
    [
        "rust/loopflow/src/ops/task.rs",
        "tests/fixtures/dto/work_first_entry.json",
        "swift/LoopflowMac/WorkModel.swift",
        "scripts/desktop_performance.py",
        "python/tests/test_desktop_performance.py",
        ".github/workflows/ci.yml",
        ".lf/config.yaml",
        "rust/loopflow/README.md",
        "Cargo.lock",
    ],
)
def test_anything_the_macos_jobs_can_reach_runs_them(path: str) -> None:
    assert affects_macos(["docs/config.md", path])


def test_an_unknown_diff_runs_macos() -> None:
    assert affects_macos([])
