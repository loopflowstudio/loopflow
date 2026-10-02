# Task operation and Desktop handoff

2026-10-02 — Jack Heart requested a skill that reads the design and scratch
history, preserves an existing Task Flow or chooses an appropriate one using the
Project default, ensures progression, and returns a Loopflow Desktop command.
Jack identified Desktop addressing as the intended handoff.

Accepted scope: shared task-operate skill and an optional Session query on the
existing Task URL. Opening a link preserves Task/Flow identity and uses the
existing conversation-opening path. Missing Sessions remain explicit failures;
Task-only links retain their current behavior.

Acceptance: the skill handles new, active, reviewing, blocked, and completed work
without duplicate execution; Desktop resolves a repository-qualified Task and
its exact Session, including paginated records, with a retryable missing-session
error that preserves the current workspace.

Review: session association uses the runtime Task identity, not the planning
Issue identity. Existing Task URLs retain their behavior; Session links use the
same conversation-opening path as the palette. The skill preserves live drivers,
review boundaries, and completed outcomes across repeated invocations.

Checks: headless WorkspaceDestinationTests (12 tests), builtin skill discovery,
and built CLI help passed; no live Task was launched or Desktop installation changed.
