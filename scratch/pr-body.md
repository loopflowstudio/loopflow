## Usage

```text
Open Loopflow → All Repos → Roadmap
Start, attach, resume, or interrupt a Task from its roadmap row.
```

## Summary

Make the default Wave Chat pane consume the shared `lf roadmap --json` snapshot across every registered Wave. Durable plan rows remain visible for stopped Waves, per-Wave read failures retain their evidence, and whole-query refresh failures keep the last successful roadmap on screen.

## Changes

- render the canonical Wave → Project → Task hierarchy with its server-derived sections
- route Wave open/start and Task run/attach/resume/interrupt controls through existing lifecycle commands
- reuse the Task workspace for agent attachment, diffs, files, shells, PRs, and worktree opening
- cover roadmap action selection and exact lifecycle argv with the shared DTO fixture

## Verification

- `swift test -Xswiftc -gnone` — 103 tests
- `uv run python ../scripts/check_swift_multiplatform_boundaries.py`
- macOS `xcodebuild build-for-testing`
- live `lf roadmap --json` read across seven live/stopped Waves, including one explicit unavailable plan
