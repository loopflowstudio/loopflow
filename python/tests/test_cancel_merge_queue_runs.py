import pytest

from scripts.cancel_merge_queue_runs import pull_request_number, superseded


def _run(id: int, branch: str, status: str = "in_progress") -> dict:
    return {"id": id, "head_branch": branch, "status": status}


def test_only_this_pull_requests_active_queue_runs_are_superseded() -> None:
    runs = [
        _run(1, "gh-readonly-queue/main/pr-1607-54da279a"),
        _run(2, "gh-readonly-queue/main/pr-1607-a31b691a", status="queued"),
        _run(3, "gh-readonly-queue/main/pr-1607-e44ab917", status="completed"),
        _run(4, "gh-readonly-queue/main/pr-16070-54da279a"),
        _run(5, "gh-readonly-queue/main/pr-1640-54da279a"),
        _run(6, None),
    ]

    assert superseded(runs, 1607, keep=2) == [1]
    assert superseded(runs, 1607, keep=None) == [1, 2]
    assert superseded(runs, 1638, keep=None) == []


@pytest.mark.parametrize(
    "entry",
    ["1607", "gh-readonly-queue/main/pr-1607-54da279a8b6894adaf00b37bccd2de09f0ee6942"],
)
def test_pull_request_number_reads_numbers_and_queue_refs(entry: str) -> None:
    assert pull_request_number(entry) == 1607


def test_pull_request_number_rejects_other_refs() -> None:
    with pytest.raises(ValueError):
        pull_request_number("refs/heads/main")
