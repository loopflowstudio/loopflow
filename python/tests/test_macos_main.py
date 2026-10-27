import pytest

from scripts.macos_main import failed_jobs, find_culprit


@pytest.mark.parametrize("first_bad", range(7))
def test_bisection_finds_the_first_red_commit(first_bad: int) -> None:
    commits = [f"c{index}" for index in range(7)]
    tested: list[str] = []

    def passes(sha: str) -> bool:
        tested.append(sha)
        return commits.index(sha) < first_bad

    assert find_culprit(commits, passes) == commits[first_bad]
    assert len(tested) <= 3


def test_a_single_commit_batch_is_its_own_culprit() -> None:
    assert find_culprit(["only"], lambda sha: pytest.fail("no candidate needs a run")) == "only"


def test_failed_check_names_select_their_jobs() -> None:
    names = ["macos / macos-process-test (macos-15)", "macos / loopflow-ui-test"]
    assert failed_jobs(names) == ["macos-process-test", "loopflow-ui-test"]
