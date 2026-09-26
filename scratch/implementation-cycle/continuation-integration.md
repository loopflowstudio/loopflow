# Continuation branch comparison — 2026-09-25

Human asked whether rebasing onto
`jack-heart/restore-task-continuation-with-a` would help. Parent recommends it
after the active bounded implementation pass, before the next compress/review.
The subsequent direction to keep building authorized the local integration below;
publication and sibling mutation remain outside this pass.

Subsequent execution decision: after the human confirmed both loops, the delivery
tail and continuing the build, the parent integrates the named runtime dependency
locally as necessary implementation work. No push, remote branch change, sibling
mutation or installed-runtime change is included. The bounded implement Run has
exited; compress/review follow integration. Preserve the whole coherent Task tree
in a local checkpoint before the reversible rebase.

Read-only `lf rebase --plan jack-heart/restore-task-continuation-with-a` reports
`direct_rebase`, protected current branch, 46 unique commits and 757 changed
files. This is a strategy preview, not a prediction of conflict-free integration.

Observed sibling source and scratch:

- `durable.rs::FlowPosition` replaces flat step_index/iteration/progress with
  the shared `ExecutionCursor`; leaf/path identity is material for nested XOR.
- `engine/execution.rs` and `engine/transitions.rs` own ordinary and Task
  traversal, backward edges and human revision. Counts do not impose pass limits.
- `ops/flow_run.rs` owns standalone invocation/boundary persistence and exact
  StepToken; `ops/flow_session.rs` projects its human boundaries through the
  existing Session kind and launch machinery.
- `scratch/cursor-integration.md` records isolated recovery/traversal proof.
  `scratch/live-loopflow/final-review.md` records accepted bounded live transport
  proof. These are sibling receipts, not freshly rerun proof of this checkout.

This directly overlaps cycle 2's new Flow membership and Session naming/open
paths. Reconcile those with exact cursor/standalone boundary authority rather
than extending the old flat managed-Task model or adding adapters. Keep names
and human provenance through both managed and standalone boundary recovery.

Resolved composition question: current sibling `task/flow/pursue.yaml` contains
both `decide -> implement` and a `decide_delivery -> implement` after human demo.
The human explicitly confirmed “The 2nd loop is correct.” This supersedes the
earlier single-loop topology in the prototype. The governing design and desktop
direction now accept both edges; source also uses `design` as the human design
skill. Render actual pinned definitions, preserving separate occurrences of
loop-decide and their per-edge traversal evidence. The original prototype source
and hash receipt remain unchanged historical visual reference.

Human follow-up: “we'll have to revisit the UI.” Revisit the two-loop diagram
visually before implementing the native Flow diagram. This does not cancel the
current named-Session slice or invalidate the accepted Session hierarchy.
The subsequent instruction keeps building until the working build is ready for
that discussion. Final Advance → queue → land is now explicitly confirmed;
final Iterate returns to implement. Delivery operations must use the existing
PR authority; no live queue/landing action is authorized by this design change.

## Local integration in progress

Cycle 2 implement exited successfully before integration. The parent checkpointed
its complete coherent tree as `59a014c42`, then ran
`lf rebase --manual jack-heart/restore-task-continuation-with-a` and resolved the
46-commit replay through `lf rebase --continue`. Rebase completed locally; no
sibling dirt, push, installation, PM edit, or live Session mutation occurred.

Conflicts joined required Session Run references and canonical names with the
shared nested cursor, standalone Flow boundaries, and Complete-based human
reviews. The current integration uses cursor boundary keys for exact occurrence
comparison, prepares standalone human Runs before publication, carries names to
replacement Runs, and resolves their Work through the existing Run resolver.
The shared action contract and Swift controls now use Complete for human reviews;
following loop-decide owns Advance/Iterate. Bound ordinary flows retain their
pinned execution but leave shared edits uncommitted.

The human confirmed final Advance → queue → land. Feature now composes its existing
queue flow and `pr land -c` after pursue. A traversal proof must exercise both
return edges and final delivery without invoking any publication operation.

Fresh native proof: 18 tests in three suites pass for Session renaming, shared DTOs,
and a rejected Flow completion retaining its real responding Ghostty terminal.
Command/log: `/tmp/loo291-continuation-swift.log`. This uses fixture transport and
owned PTYs, not a configured vendor conversation or final composition acceptance.

The first integrated Rust compiles exposed missing standalone DTO fields, old
flat cursor references, and a merged CLI dispatcher tail. These are corrected;
focused runtime/CLI proofs are still running. No final Rust pass or complete
integration claim is made yet. Prior cycle-2 Rust receipts predate this rebase.

The first 17-test human-Session run stopped at keyed Ask retry: `ask_once`
required no linked Run, which prepared identity now makes false before launch.
The exact owned test/runner was terminated; it supplies no suite pass. The
correction treats an unconsumed preparation as launchable while preserving an
already-published native Run. The existing retry and concurrent-completion
regressions are being rerun. Log: `/tmp/loo291-continuation-human.log`.

## Focused integration outcomes

- `cargo test -p loopflow --lib ops::flow_session::tests -- --test-threads=1`:
  three pass; prepared identity, named failed-launch recovery, nested review
  feedback and the outer backward edge. `flow-session.log` under the paths below.
- `cargo test -p loopflow --lib feature_repeats_the_whole_slice_then_stops_for_delivery -- --test-threads=1`:
  one pass; both repeated edges, final Advance and the exact queue/landing tail.
  No publication operation is executed by this traversal fixture.
- `cargo test -p loopflow --lib ops::human_session::tests -- --test-threads=1`:
  17 pass after the Ask retry correction (`human2.log`). Includes concurrent
  completion, retained native identity, keyed recovery, action/wire fixtures.
- `cargo test -p loopflow --lib flow_sessions_are_named_through_their_run_and_keep_exact_membership -- --test-threads=1`:
  one pass; managed human and captured Run membership/naming.
- `cargo test -p loopflow --test session_cli_tests`: seven pass; real local CLI,
  isolated Homes, development-helper handoff, prelaunch identity, name protection,
  replacement and initial/resumed native clients using an owned provider stand-in.
- `cargo test -p loopflow --test flow_tests bound_flows_keep_task_context_and_leave_managed_flow_and_shared_edits_alone`:
  one pass. The public CLI executes independent Task-bound steps, preserves the
  managed Flow and shared dirt, and publishes a standalone human review with
  exact Task attribution before starting a provider. Rename by that Run ID and
  JSON reopen retain the same boundary, Run, human name, and Work.

Log prefix is `/tmp/loo291-continuation-`; the source receipt will retain exact
paths/hashes. Native proof remains 18 tests in three suites (`swift.log`). The
first all-target clippy found one redundant test borrow, now removed; the final
`cargo clippy --all-targets -- -D warnings` passes (`clippy2.log`). Formatting and
whitespace checks pass. No broader gate, hosted UI, configured vendor demo, remote-Home
trial, or final two-loop visual acceptance is claimed. Source review confirms
no obsolete publish_run_binding, require_flow_decision, Swift decideFlow or
resolveFlowSession call remains. The next passes are independent compress then
review-slice, before the next presentation implementation.

## Delegation transport

The Claude compress launch failed before provider execution with `Argument list
too long (os error 7)`: the assembled context reports 237,610 tokens, including
217,768 scratch tokens. The current one-shot Claude path places task_prompt in
argv. No scratch was removed to fit it. The existing Codex batch path uses the
app-server message transport, so the parent uses that supported lf provider path
for this implementation review contribution. This changes no installed provider
configuration or runtime code. Original failure log:
`/tmp/loo291-parent-cycle02-compress-output.log`.

The Codex alternative also failed before a turn: its raw provider response to
request 3 reports `input_too_large`, max_chars 1048576, actual_chars 1158726. The
older installed launcher did not propagate that rejected turn-start response,
so its exact owned lf/app-server pair remained waiting. Parent stopped PIDs
64753/65410 after verifying their Run and parent ownership; no unclaimed/user
provider was touched. This is a second transport boundary, not slow reasoning.
Run: `run_85554e0f873b415693e39399632577b9`. No compress result exists yet.

Parent is correcting the existing Claude batch command transport locally:
an anonymous private file supplies task text as stdin, with system context using
the existing prompt file. Both streaming and captured-output launch modes must
preserve an oversized multiline Unicode prompt byte-for-byte. No context cutoff,
second launch authority, account/config change or scratch removal is proposed.
The ignored Codex turn-start rejection is retained as a separate diagnostic;
this pass does not claim to repair that driver.

The transport regression fails before the fix with the same OS error and passes
afterward: both streaming and captured output receive the complete ~1.5 MB
multiline Unicode prompt through an owned stand-in process. Logs:
`/tmp/loo291-large-prompt-before.log` and `-after.log`. The implementation changes
only Claude automatic launches: an anonymous private file supplies stdin; both
existing output runners retain it. Interactive and other-provider argv behavior
is preserved. Final focused timeout checks and clippy precede the configured
retry through this checkout's `scripts/dev-lf`; no installation is needed.

## Successful delegated continuation

The local helper retry could not resolve Task Work: its selected development
store at `~/.lf-dev/worktrees/loopflow-main-view-task-6787ccf6a153/loopflow.db`
rejects the changed canonical migration frontier before `0.12.20.001_release`.
No fresh promotion, database reset, production override, or installation occurred.

After a local checkpoint through `lf commit`, parent preserved three fully
superseded chronology documents as byte-identical adjacent `*-archive.txt` files
and replaced their original Markdown paths with short indexes. Every old link
still reaches an index, all source text and evidence remain available, and the
current design/receipts stay in the automatic Markdown snapshot. Hash receipt:
[historical-context-archive.json](historical-context-archive.json). Total archived
text: 219,458 bytes. This is explicit context curation after the transport
counterexamples, not an omitted observation or discarded design requirement.

The installed `lf --task LOO-291 -b -m claude --no-diff --no-diff-files compress`
now launches successfully with 196,653 assembled tokens and emits real model
inspection output. Log: `/tmp/loo291-parent-cycle02-compress-final-output.log`.
This final contribution uses installed lf, superseding the brief's earlier
local-helper retry statement. The local stdin correction retains its three
passing stand-in/timeout checks and clean clippy, but no configured vendor use
of that uninstalled correction is claimed. Parent continues with independent
review after this compress Run exits.
