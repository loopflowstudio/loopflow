import json
from pathlib import Path

from scripts.context_ablation import ARMS, applicable_arms, census, observe

_PROMPT = """<lf:wave-memory>
{memory}
</lf:wave-memory>

<lf:scratch>
<lf:file path="scratch/questions.md">
open question
</lf:file>
<lf:file path="scratch/add-a-thing.md">
the design
</lf:file>
<lf:file path="scratch/parent/notes.md">
stacked parent notes
</lf:file>
</lf:scratch>

<lf:work kind="task" id="task_1">
directive
<lf:steers>
- earlier direction
</lf:steers>
</lf:work>

Implement it.
"""


def _manifest(memory: str = "remembered") -> dict:
    return {
        "created_at": "2026-10-01T10:00:00Z",
        "skill": "implement",
        "worktree": "/src/repo.add-a-thing",
        "launch": {"task_prompt": _PROMPT.format(memory=memory)},
    }


def test_each_arm_removes_only_its_source():
    manifest = _manifest()
    prompt = manifest["launch"]["task_prompt"]

    without_memory = ARMS["no-memory"](prompt, manifest)
    assert "remembered" not in without_memory and "the design" in without_memory

    without_scratch = ARMS["no-scratch"](prompt, manifest)
    assert "the design" not in without_scratch and "remembered" in without_scratch

    own = ARMS["own-scratch"](prompt, manifest)
    assert "the design" in own and "open question" in own
    assert "stacked parent notes" not in own

    without_steers = ARMS["no-steers"](prompt, manifest)
    assert "earlier direction" not in without_steers and "directive" in without_steers
    assert without_steers.rstrip().endswith("Implement it.")


def test_trimmed_memory_keeps_both_ends_and_small_memory_is_not_an_arm():
    long = "first decision\n" + "filler line\n" * 4_000 + "latest decision"
    manifest = _manifest(long)
    trimmed = ARMS["trim-memory"](manifest["launch"]["task_prompt"], manifest)
    assert "first decision" in trimmed and "latest decision" in trimmed
    assert "Excerpt only" in trimmed
    assert len(trimmed) < len(manifest["launch"]["task_prompt"]) / 2

    assert "trim-memory" not in applicable_arms(_manifest())
    assert "trim-memory" in applicable_arms(manifest)


def test_census_weighs_sources_and_keeps_missing_usage_absent(tmp_path: Path):
    directory = tmp_path / "ab" / "run_ab"
    directory.mkdir(parents=True)
    manifest = _manifest()
    task = manifest["launch"]["task_prompt"]
    (directory / "manifest.json").write_text(json.dumps(manifest))
    (directory / "terminal.json").write_text(
        json.dumps({"outcome": "completed", "ended_at": "2026-10-01T10:06:00Z"})
    )
    assets = [
        {"kind": "memory", "label": "wave memory", "attributed_tokens": 600},
        {"kind": "scratch", "label": "scratch/add-a-thing.md", "attributed_tokens": 100},
        {"kind": "scratch", "label": "scratch/parent/notes.md", "attributed_tokens": 300},
    ]
    context = {"system": None, "task": {"text": task, "tokens": 1_000, "assets": assets}}
    (directory / "context.json").write_text(json.dumps({"context": context}))
    command = {"type": "command", "command": ["/bin/zsh -c 'cargo test && lf flow advance'"]}
    events = [
        {"type": "user_input", "op": "initial"},
        {"type": "conversation", "event": {"type": "item_completed", "item": command}},
        {"type": "usage", "usage": {"cost_usd": 1.5}},
    ]
    (directory / "events.jsonl").write_text("\n".join(json.dumps(event) for event in events))

    step = census(tmp_path)["steps"]["implement"]
    assert step["records"] == 1
    assert step["sources"]["memory"]["share_of_submitted"] == 0.6
    assert step["sources"]["scratch_own"]["sum"] == 100
    assert step["sources"]["scratch_older"]["sum"] == 300
    assert step["turns"]["minutes"]["median"] == 6
    assert "peak_input_tokens" not in step["turns"]

    turn = observe(directory)
    assert turn["decision"] == "advance"
    assert len(turn["checks"]) == 1
    assert turn["peak_input_tokens"] is None
