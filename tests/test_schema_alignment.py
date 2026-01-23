"""Tests to ensure schema alignment between Python, Swift, and TypeScript models.

Python models are the source of truth. Swift and TypeScript should mirror them exactly.
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


def extract_typescript_fields(interface_name: str, file_path: Path) -> set[str]:
    """Extract field names from a TypeScript interface."""
    content = file_path.read_text()

    # Find the interface definition
    pattern = rf"export interface {interface_name}\s*\{{([^}}]+)\}}"
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        return set()

    interface_body = match.group(1)
    fields = set()

    # Match field declarations: name: Type or name?: Type
    field_pattern = r"(\w+)\??\s*:"
    for match in re.finditer(field_pattern, interface_body):
        fields.add(match.group(1))

    return fields


# Field name mappings for snake_case (Python) to camelCase (Swift/TS)
PYTHON_TO_CAMEL = {
    "started_at": "startedAt",
    "ended_at": "endedAt",
    "run_mode": "runMode",
    "flow_run_id": "flowRunId",
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


class TestStepRunSchema:
    """StepRun schema alignment tests."""

    def test_steprun_python_fields(self):
        """Python StepRun has expected fields."""
        fields = extract_python_fields(
            "StepRun", REPO_ROOT / "src/loopflow/lfd/models.py"
        )
        # Core fields that must exist
        assert "id" in fields
        assert "step" in fields
        assert "repo" in fields
        assert "worktree" in fields
        assert "status" in fields

    def test_steprun_swift_matches_python(self):
        """Swift StepRun fields match Python schema."""
        python_fields = extract_python_fields(
            "StepRun", REPO_ROOT / "src/loopflow/lfd/models.py"
        )
        swift_fields = extract_swift_fields(
            "StepRun", REPO_ROOT / "swift/LoopflowCore/Models/Step.swift"
        )

        python_camel = to_camel_case(python_fields)

        # Swift should have these core fields from Python
        core_fields = {"id", "step", "repo", "worktree", "status", "startedAt", "model", "runMode"}
        missing = core_fields - swift_fields
        assert not missing, f"Swift StepRun missing fields: {missing}"

    def test_steprun_typescript_matches_python(self):
        """TypeScript StepRun fields match Python schema."""
        python_fields = extract_python_fields(
            "StepRun", REPO_ROOT / "src/loopflow/lfd/models.py"
        )
        ts_fields = extract_typescript_fields(
            "StepRun", REPO_ROOT / "web/src/models/step.ts"
        )

        # TypeScript should have these core fields from Python
        core_fields = {"id", "step", "repo", "worktree", "status", "startedAt", "model", "runMode"}
        missing = core_fields - ts_fields
        assert not missing, f"TypeScript StepRun missing fields: {missing}"


class TestAgentSchema:
    """Agent schema alignment tests."""

    def test_agent_python_fields(self):
        """Python Agent has expected fields."""
        fields = extract_python_fields(
            "Agent", REPO_ROOT / "src/loopflow/lfd/models.py"
        )
        assert "id" in fields
        assert "repo" in fields
        assert "flow" in fields
        assert "voice" in fields
        assert "area" in fields
        assert "status" in fields

    def test_agent_swift_matches_python(self):
        """Swift Agent fields match Python schema."""
        python_fields = extract_python_fields(
            "Agent", REPO_ROOT / "src/loopflow/lfd/models.py"
        )
        swift_fields = extract_swift_fields(
            "Agent", REPO_ROOT / "swift/LoopflowCore/Models/Agent.swift"
        )

        python_camel = to_camel_case(python_fields)

        # Swift should have these core fields
        core_fields = {"id", "repo", "flow", "voice", "area", "status", "iteration"}
        missing = core_fields - swift_fields
        assert not missing, f"Swift Agent missing fields: {missing}"

    def test_agent_typescript_matches_python(self):
        """TypeScript Agent fields match Python schema."""
        ts_fields = extract_typescript_fields(
            "Agent", REPO_ROOT / "web/src/models/agent.ts"
        )

        core_fields = {"id", "repo", "flow", "voice", "area", "status", "iteration"}
        missing = core_fields - ts_fields
        assert not missing, f"TypeScript Agent missing fields: {missing}"


class TestFlowRunSchema:
    """FlowRun schema alignment tests."""

    def test_flowrun_python_fields(self):
        """Python FlowRun has expected fields."""
        fields = extract_python_fields(
            "FlowRun", REPO_ROOT / "src/loopflow/lfd/models.py"
        )
        assert "id" in fields
        assert "flow" in fields
        assert "repo" in fields
        assert "status" in fields

    def test_flowrun_swift_matches_python(self):
        """Swift FlowRun fields match Python schema."""
        swift_fields = extract_swift_fields(
            "FlowRun", REPO_ROOT / "swift/LoopflowCore/Models/FlowRun.swift"
        )

        core_fields = {"id", "flow", "repo", "status", "iteration"}
        missing = core_fields - swift_fields
        assert not missing, f"Swift FlowRun missing fields: {missing}"

    def test_flowrun_typescript_matches_python(self):
        """TypeScript FlowRun fields match Python schema."""
        ts_fields = extract_typescript_fields(
            "FlowRun", REPO_ROOT / "web/src/models/flow-run.ts"
        )

        core_fields = {"id", "flow", "repo", "status", "iteration"}
        missing = core_fields - ts_fields
        assert not missing, f"TypeScript FlowRun missing fields: {missing}"
