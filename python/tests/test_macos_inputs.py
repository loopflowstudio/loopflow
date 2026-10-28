import pytest

from scripts.macos_inputs import (
    JOBS,
    LOOPFLOW_UI_TEST,
    MACOS_PROCESS_TEST,
    SWIFT_TEST,
    affected_jobs,
)

_FILES = {
    "rust/loopflow/src/ops/task.rs": "pub fn run() {}\n",
    "rust/loopflow/src/app_process.rs": '#[cfg(target_os = "macos")]\nfn ps() {}\n',
}


def _jobs(*paths: str) -> set[str]:
    return affected_jobs(list(paths), _FILES.get)


def test_rust_only_change_runs_no_macos_job() -> None:
    assert _jobs("rust/loopflow/src/ops/task.rs", "Cargo.lock", "docs/config.md") == set()


@pytest.mark.parametrize(
    "path", ["swift/LoopflowMac/WorkModel.swift", "swift/project.yml", "swift/GhosttyKitPatches/a"]
)
def test_swift_change_runs_every_swift_job(path: str) -> None:
    assert _jobs(path) == {SWIFT_TEST, LOOPFLOW_UI_TEST}


@pytest.mark.parametrize(
    "path",
    [
        "tests/fixtures/dto/work_first_entry.json",
        "python/tests/test_desktop_performance.py",
        "scripts/swift_test_limit.py",
    ],
)
def test_wire_fixtures_and_swift_scripts_run_swift_test(path: str) -> None:
    assert _jobs(path) == {SWIFT_TEST}


@pytest.mark.parametrize(
    "path",
    [
        "rust/loopflow/src/app_process.rs",
        "rust/loopflow/src/store/copy.rs",
        "rust/loopflow/src/lf/commands/install/published.rs",
    ],
)
def test_macos_rust_runs_the_process_test(path: str) -> None:
    assert _jobs(path) == {MACOS_PROCESS_TEST}


def test_deleted_rust_file_without_text_skips_the_process_test() -> None:
    assert _jobs("rust/loopflow/src/gone.rs") == set()


@pytest.mark.parametrize("path", [".github/workflows/ci.yml", "scripts/macos_inputs.py"])
def test_ci_changes_run_every_job(path: str) -> None:
    assert _jobs("docs/config.md", path) == set(JOBS)


@pytest.mark.parametrize("paths", [None, []])
def test_an_unknown_diff_runs_every_job(paths: list[str] | None) -> None:
    assert affected_jobs(paths, _FILES.get) == set(JOBS)
