# Interactive review feedback and explicit double loops

2026-09-25. Implemented the human's final clarification: scheduled review is
`demo human:true`, followed by loop-decide; Ask handles blocked execution.
Both deciding occurrences in pursue explicitly target implement. Review can
clarify or replace the design before returning feedback. Completion of the
conversation is not a claim of successful behavior or a navigation verdict.

The concept-review exercise began with usage/schema and followed the simpler
experience through APIs and storage. It identified and removed inferred human
revision, Session Advance/Iterate, Task advance's approval arguments, separate
ordinary human-verdict settlement, Task human Iterate/Steer handling, Swift
approval dialogs/commands, an unused unsaved executor mode, duplicate
task-kickoff composition, and unused local XOR/live-demo scaffolding. Native
review launch, history, readiness and exact completion remain necessary for
human:true. No new lifecycle or feedback store was added.

Completion saves the ready summary before continuation/cleanup. Ordinary Flow
completion stays on its saved boundary until the common engine consumes it.
Task completion reads feedback from the exact position it atomically settles.
Both carry feedback through cursor direction to the next step. loop-decide
reads that summary and revised artifacts, then records its own decision.

## Evidence

Commands below cleared ambient LF_CONTROL_HOME, LF_CONTROL_DB_PATH, LF_HOME,
LF_DB_PATH, LF_RUN_CONTEXT, LF_RUN_ID, LF_RUN_DIR, LF_FLOW_STEP and
LF_HUMAN_SESSION. Stores and provider effects were isolated.

- Initial focused library selection: **104 passed**. Included transition,
  execution, Flow expansion, ordinary persistence/review, CLI parsing/driver,
  builtin and selected Task tests. This was an intermediate tree.
- Subsequent **38-test** selection: **36 passed, 2 failed**. Ask joining,
  retained completion, cleanup-failure and reopen races passed, as did the Task
  fresh-Run driver and actual next-prompt feedback. A builtin substring still
  required obsolete demo acceptance wording; the recovery fixture registered
  the already-running test process as a newly started client, invalidating its
  five-second start-time evidence. Updated the contract assertion and replaced
  that fixture with a freshly spawned, owned sleep child with scoped cleanup.
  No retry or production liveness relaxation was introduced.
- Final affected regression selection: **8 passed**, including both repaired
  tests, ordinary nested feedback/recovery, Task nested feedback in the actual
  prepared loop-decide input, missing readiness, stale completion, concurrent
  completion, and final-review completion. Task feedback is read from the saved
  position, rather than a caller-supplied summary that could race readiness.
- `cargo test -p loopflow --test session_cli_tests`: **2 passed**. Real CLI
  help/resolution and development executable/Home handoff remain correct.
- Extended the actual `CliFlowExecutor` test to complete the retained review
  through the public adapter and resume it through drive_saved: **1 passed**.
  Ready alone stays waiting; Complete stores feedback without a verdict;
  ordinary execution finishes once and repeated resume does no work.
- `swift test --package-path swift -Xswiftc -gnone --filter SessionsStoreTests`:
  **12 passed**; the shared library and Mac app compiled. Complete removes the
  review after refresh. Hosted UI assertions now expect Complete for all three
  Session kinds and absence of Advance/Iterate; the hosted UI test was not run.

These selections overlap; their counts are not distinct coverage to sum.
Architecture ownership, guide YAML/fences, skill frontmatter, formatting and
whitespace checks passed. Final all-target Clippy result is recorded below.

The exact saved-definition and stale-Run checks remain; no pass limit or nearest
step heuristic remains. This is local implementation proof, not a new live
provider/human demonstration. No installation or production Session deletion
was performed. The earlier accepted transport demo predates this changed model.
The human explicitly waived migrating old review conversations; obsolete saved
human navigation may require a fresh invocation.

A concurrent `lf pr open` checkpoint eeed65c43 captured most implementation and
removed the working concept-review note. This Run did not initiate that
publication. The last CLI behavioral extension and this evidence are separate
working-tree edits. Wave interpreter removal remains the distinct journal,
resident and client change described in wave-playhead-removal.md.

Final `cargo clippy --all-targets -- -D warnings` passed. Final
`cargo fmt --all -- --check` and `git diff --check` passed.
