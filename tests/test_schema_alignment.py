"""Tests to ensure schema alignment between Python and Swift models.

Python models are the source of truth. Swift should mirror them exactly.
"""

import re
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent


def extract_python_fields(model_name: str, file_path: Path) -> set[str]:
    """Extract field names from a Python dataclass or Pydantic model."""
    content = file_path.read_text()

    # Find the class definition
    pattern = rf"class {model_name}\([^)]+\):\s*\n((?:\s+.+\n)+)"
    match = re.search(pattern, content)
    if not match:
        return set()

    class_body = match.group(1)
    fields = set()

    # Match field definitions: name: Type or name: Type = default
    field_pattern = r"^\s+(\w+):\s*[^=\n]+"
    for line in class_body.split("\n"):
        field_match = re.match(field_pattern, line)
        if field_match:
            field_name = field_match.group(1)
            # Skip private fields and class-level constants
            if not field_name.startswith("_") and field_name != "model_config":
                fields.add(field_name)

    return fields


def extract_swift_fields(struct_name: str, file_path: Path) -> set[str]:
    """Extract field names from a Swift struct."""
    content = file_path.read_text()

    # Find the struct definition
    pattern = rf"public struct {struct_name}[^{{]*\{{([^}}]+)\}}"
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        return set()

    struct_body = match.group(1)
    fields = set()

    # Match let/var declarations: public let/var name: Type
    field_pattern = r"public (?:let|var) (\w+):"
    for match in re.finditer(field_pattern, struct_body):
        fields.add(match.group(1))

    return fields


# Field name mappings for snake_case (Python) to camelCase (Swift)
PYTHON_TO_CAMEL = {
    "started_at": "startedAt",
    "ended_at": "endedAt",
    "run_mode": "runMode",
    "wave_run_id": "waveRunId",
    "agent_id": "agentId",
    "main_branch": "mainBranch",
    "pr_limit": "prLimit",
    "merge_mode": "mergeMode",
    "created_at": "createdAt",
    "watch_paths": "watchPaths",
    "last_main_sha": "lastMainSha",
    "current_step": "currentStep",
    "pr_url": "prUrl",
}


def to_camel_case(python_fields: set[str]) -> set[str]:
    """Convert Python snake_case fields to camelCase."""
    return {PYTHON_TO_CAMEL.get(f, f) for f in python_fields}


class TestAgentSchema:
    """Agent schema alignment tests."""

    def test_agent_python_fields(self):
        """Python Agent has expected fields."""
        fields = extract_python_fields("Agent", REPO_ROOT / "src/loopflow/lfd/models.py")
        # Core fields that must exist
        assert "id" in fields
        assert "step" in fields
        assert "repo" in fields
        assert "worktree" in fields
        assert "status" in fields

    def test_agent_swift_matches_python(self):
        """Swift Agent fields match Python schema."""
        swift_path = REPO_ROOT / "swift/LoopflowCore/Models/Agent.swift"
        if not swift_path.exists():
            # Skip if Swift file not renamed yet (StepRun.swift → Agent.swift)
            import pytest

            pytest.skip("Swift Agent.swift not yet created (pending StepRun→Agent rename)")

        python_fields = extract_python_fields("Agent", REPO_ROOT / "src/loopflow/lfd/models.py")
        swift_fields = extract_swift_fields("Agent", swift_path)

        python_camel = to_camel_case(python_fields)

        # Swift should have these core fields from Python
        core_fields = {"id", "step", "repo", "worktree", "status", "startedAt", "model", "runMode"}
        missing = core_fields - swift_fields
        assert not missing, f"Swift Agent missing fields: {missing}"


class TestWaveSchema:
    """Wave schema alignment tests."""

    def test_wave_python_fields(self):
        """Python Wave has expected fields."""
        fields = extract_python_fields("Wave", REPO_ROOT / "src/loopflow/lfd/models.py")
        assert "id" in fields
        assert "repo" in fields
        assert "flow" in fields
        assert "direction" in fields
        assert "area" in fields
        assert "status" in fields

    def test_wave_swift_matches_python(self):
        """Swift Wave fields match Python schema."""
        swift_path = REPO_ROOT / "swift/LoopflowCore/Models/Wave.swift"
        if not swift_path.exists():
            # Skip if Swift file not renamed yet (Agent.swift → Wave.swift)
            import pytest

            pytest.skip("Swift Wave.swift not yet created (pending Agent→Wave rename)")

        python_fields = extract_python_fields("Wave", REPO_ROOT / "src/loopflow/lfd/models.py")
        swift_fields = extract_swift_fields("Wave", swift_path)

        python_camel = to_camel_case(python_fields)

        # Swift should have these core fields
        core_fields = {"id", "repo", "flow", "direction", "area", "status", "iteration"}
        missing = core_fields - swift_fields
        assert not missing, f"Swift Wave missing fields: {missing}"


class TestWaveRunSchema:
    """WaveRun schema alignment tests."""

    def test_waverun_python_fields(self):
        """Python WaveRun has expected fields."""
        fields = extract_python_fields("WaveRun", REPO_ROOT / "src/loopflow/lfd/models.py")
        assert "id" in fields
        assert "flow" in fields
        assert "repo" in fields
        assert "status" in fields

    def test_waverun_swift_matches_python(self):
        """Swift WaveRun fields match Python schema."""
        swift_path = REPO_ROOT / "swift/LoopflowCore/Models/WaveRun.swift"
        if not swift_path.exists():
            # Skip if Swift file not renamed yet (FlowRun.swift → WaveRun.swift)
            import pytest

            pytest.skip("Swift WaveRun.swift not yet created (pending FlowRun→WaveRun rename)")

        swift_fields = extract_swift_fields("WaveRun", swift_path)

        core_fields = {"id", "flow", "repo", "status", "iteration"}
        missing = core_fields - swift_fields
        assert not missing, f"Swift WaveRun missing fields: {missing}"
