from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import MergeMode, Wave, WaveStatus
from loopflow.lfd.protocol_v1 import wave_to_proto, worktree_to_proto


def test_wave_to_proto_maps_enums(tmp_path: Path) -> None:
    """Wave to proto maps enum values correctly.

    Note: stimulus is now a separate entity, not part of Wave.
    """
    wave = Wave(
        id="wave_1",
        name="swift-falcon",
        repo=tmp_path,
        flow="ship",
        direction=["product-engineer"],
        area=["src/api"],
        paused=False,
        status=WaveStatus.IDLE,
        iteration=2,
        pr_limit=5,
        merge_mode=MergeMode.PR,
        created_at=datetime(2026, 1, 27, 10, 0, 0),
    )

    payload = wave_to_proto(wave)

    assert payload["status"] == "WAVE_IDLE"
    assert payload["merge_mode"] == "MERGE_PR"
    assert payload["created_at"].endswith("Z")
    # stimulus is no longer on Wave proto - it's a separate message


def test_worktree_to_proto_maps_status() -> None:
    worktree_state = {
        "branch": "feat/auth",
        "path": "/tmp/repo/feat/auth",
        "base_branch": "main",
        "working_tree": {
            "staged": True,
            "modified": False,
            "untracked": True,
            "diff_vs_main": {"added": 5, "deleted": 3},
        },
        "main": {"ahead": 1, "behind": 2},
        "remote": {"name": "origin", "ahead": 0, "behind": 1},
        "operation_state": "rebase",
        "ci": {"source": "pr", "url": "https://example.com/pr/1", "state": "open"},
        "prunable": True,
        "staleness": "merged",
        "recent_steps": [
            {
                "id": "step_1",
                "step": "design",
                "status": "completed",
                "startedAt": "2026-01-01T00:00:00Z",
                "endedAt": "2026-01-01T01:00:00Z",
            }
        ],
    }

    payload = worktree_to_proto(worktree_state)

    assert payload["working_tree"]["diff_added"] == 5
    assert payload["working_tree"]["diff_deleted"] == 3
    assert payload["operation_state"] == "OPERATION_REBASE"
    assert payload["staleness"] == "STALENESS_MERGED"
    assert payload["recent_steps"][0]["status"] == "STEP_COMPLETED"
