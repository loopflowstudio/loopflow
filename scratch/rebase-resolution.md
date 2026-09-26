# Rebase resolution — 2026-09-25

Completed the existing Loopflow-owned rebase onto pinned main `1a691ac6a`
through `lf rebase --continue`. No new rebase, delegation or push.

- WorkSurfaceView retains the branch's deletion of its unused terminal store
  and unreachable inspector sheet. Main's change to that deleted store's
  ownership does not restore a consumer.
- Wave playhead/runtime retain main's interpreter removal. Incoming changes
  to the retired root invocation and its tests do not restore that interpreter.
- Ordinary Flow dispatch retains the branch's optional WorkBinding and uses
  main's current execute signature, without the deleted singleton-step argument.

Focused proof after replay:

- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
  WorkspaceNavigationTests/inspectorShowsPlanning`: one test passed, including
  Task directive and Wave chapter detail rendering.
- `cargo test -p loopflow --test flow_tests
  bound_flows_keep_task_context_and_leave_managed_flow_and_shared_edits_alone
  -- --exact`: one test passed, with ambient Home/Run/Work/Session authority
  cleared. Real CLI dispatch uses an isolated store and simulated provider.
  It preserves bound context, managed position and shared edits, parks at the
  human review, and retains Session identity through rename/open.

The Rust proof first exposed two stale test references: QueuedInvocation's old
Wave module and the removed membership `current` boolean. Updated the test to
the engine module and `occurrence: "current"`; the final rerun passed in 53.18s.
These two test corrections remain uncommitted for the waiting parent, alongside
this note. Logs: `/tmp/loopflow-rebase-swift-proof.log` and
`/tmp/loopflow-rebase-rust-proof.log`.

No broader gate, live provider or native terminal acceptance is claimed. The
waiting parent owns final Git postconditions and publication.
