from pathlib import Path

from generate_screenshots import load_shots, shot_environment


def test_fixture_captures_cover_three_states_at_both_widths(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("LOOPFLOW_UI_TEST_DETAIL_STATE", "error")
    monkeypatch.setenv("LOOPFLOW_UI_TEST_VIEW", "portfolio")
    captures = []
    for shot in load_shots():
        output = tmp_path / f"{shot.name}.png"
        env = shot_environment(shot, output)
        assert env["LOOPFLOW_UI_TEST_SNAPSHOT_PATH"] == str(output)
        assert "LOOPFLOW_UI_TEST_VIEW" not in env
        captures.append(
            (
                env["LOOPFLOW_UI_TEST_WIDTH"],
                env["LOOPFLOW_UI_TEST_MODE"],
                env.get("LOOPFLOW_UI_TEST_DETAIL_STATE"),
            )
        )

    assert sorted(captures) == sorted(
        (width, mode, state)
        for width in ("900", "1440")
        for mode, state in (
            ("mock-waves", "loading"),
            ("mock-waves", "error"),
            ("empty-workspaces", None),
        )
    )
