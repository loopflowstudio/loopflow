# Required Session → Run — implementation slice

Follow-up: [recovery review and correction](session-run-review.md) reproduces and
fixes a launch-lock regression in native resume. The expanded CLI proof covers
both first launch and resume with a local stand-in provider. Its receipts supersede
the earlier "no executable edits followed" statement for the changed launch path.

## Remaining work

Use required `SessionRecord.run_id` / Swift `runId` for the future Monitor join.
It identifies a Run, not a live process. The active-Run projection still needs
the exact ownership/Task joins described in
[the model review](research-workspace-model-7eefb9b3.md). Watch is not adopted as
that model. LOO-293 and its checkout remain untouched.

The full canvas, mixed Monitor/terminal panes, `hierarchy_interaction_ms` and
`task_workspace_ready_ms` remain the complete target in
[the canvas design](canvas-chapter-review.md). Concurrent outline edits in this
shared checkout are preserved; this slice does not claim their implementation
or validation. No configured provider trial, installation, PM mutation, Task
completion or publication is part of this pass.

## Observable contract

An unopened Ask/Flow Session already names a resolvable Run. Preparing it starts
no provider and supplies no liveness evidence. Launch consumes that preparation
once, filling in launch context/provenance without changing its Run identity.
Native resume continues to use the linked Run. Ask ancestry names the caller's
Run separately from the human Session's Run. Interactive Sessions retain equal
Session and Run IDs; Ask/Flow retain their existing exact boundary IDs.

Run capture owns preparation in the existing Run directory. Flow transitions
and Ask publication establish the reference before publishing the boundary.
The child consumes the prepared identity; the temporary Run-binding file and
post-launch identity writes are removed. In particular, startup no longer
clears a Ready summary that a fast child has already published.

Internal Flow positions still have an optional reference because autonomous
steps have no human Session. Old persisted unbound boundaries are upgraded by
`lf session open <id> --json`, without provider launch. Listing never writes;
it fails explicitly with that recovery command instead of returning a Session
with no Run. This affects the whole list response until the old boundary is
prepared. No live records were migrated by this pass.

## Review findings

- Preparing identity and starting a client are separate operations. A prepared
  manifest must not acquire a failed terminal receipt when preparation returns.
- Recovering a non-resumable or missing Run publishes a replacement reference
  before starting its child; the returned open receipt carries that replacement.
  A retained provider conversation resumes its existing Run.
- The Run's initial preparation does not freeze prompt/model/runtime provenance;
  launch finalizes it once. A second consumer is rejected by atomic marker claim.
- Boundary publication and Run preparation span existing storage boundaries.
  A failed boundary write can leave an unreferenced prepared Run. It has no
  provider owner and must not appear as active work. This slice adds no second
  registry or speculative process inference to conceal that distinction.
- Shared required-field fixtures cover Rust and Swift. Session action dispatch
  continues to use boundary IDs; no client parses IDs to discover Runs.

## Focused proof

- `run_record::tests::prepared_session_run_is_resolvable_and_consumed_once`:
  prelaunch lookup, parent identity, no terminal/client, same Run after launch
  preparation, one consumer, one final record. One test passed;
  `/tmp/loo291-prepared-run-proof.log`. This is local capture proof, not a native
  provider launch.
- CLI `unopened_session_has_a_run_before_any_provider_is_started`: read-only
  legacy failure, explicit preparation, required reference, repeated open/list
  preserving identity and no client/events. One test passed;
  `/tmp/loo291-session-cli-proof.log`.
- Flow `claimed_autonomous_boundary_settles_once_at_the_human_node`: prepared
  Run exists when the human step becomes visible; revisit preserves identity
  and playhead version. One test passed;
  `/tmp/loo291-flow-prepared-proof.log`.
- `ops::human_session::tests`: eleven tests passed, including required `run_id`
  for Interactive/Ask/Flow and rejection of missing/null fields;
  `/tmp/loo291-session-run-final-tests.log`.
- Swift `DTOFixtureTests/(sessionRequiresRun|sessionsFixtureRoundTrips|flowSessionFixtureRoundTrips)`:
  three tests passed; `/tmp/loo291-session-run-swift.log`. Uses
  `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter …`.

- The `human_node` filter passed eight tests, including restart, human-boundary
  preparation, approval and iteration; `/tmp/loo291-flow-run-final-tests.log`.
- Final `cargo test -p loopflow --test session_cli_tests` passed both tests
  after removing unnecessary preparation from ordinary interactive opens;
  `/tmp/loo291-session-cli-final-proof.log`.
- Final `cargo clippy --all-targets -- -D warnings` passed;
  `/tmp/loo291-session-run-final-clippy.log`. `cargo fmt` and
  `git diff --check` pass. No executable edits followed these final checks.

No broader gate is claimed. The initial shared-tree checkpoint is `32c1ed6ef`; this contribution
remains uncommitted alongside ongoing outline work rather than claiming another
writer's edits in a final checkpoint.
