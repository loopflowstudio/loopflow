# Deduplicate captured XOR cursor bodies

Bounded LOO-295 contribution, 2026-09-25. Owns engine/execution.rs,
the required store/sqlite/durable.rs test initializer update, and this note.
The shared human review notes and main's recovery files remain untouched.

Finish line: a selected XOR cursor stores only its name and child cursor;
all traversal uses its enclosing captured ConcreteXor. Saved current records
resume with their pending decision, counts, direction and nested position after
source deletion. A missing captured parent/path must not execute an old child
copy or silently settle the selection.

## Required patch for main's owned file

The initializer in ops/flow_session.rs::tests::
saved_nested_human_review_requires_exact_approval_and_recovers_once needs:

```diff
--- a/rust/loopflow/src/ops/flow_session.rs
+++ b/rust/loopflow/src/ops/flow_session.rs
@@
             run.cursor.child = Some(Box::new(NestedCursor::Xor {
                 selected: "chosen".into(),
-                steps: child,
                 cursor: ExecutionCursor {
```

Its captured ConcretePath already contains child.clone(). Optionally change
that initializer to `steps: child` once the removed cursor use is gone.
No production caller signatures change; no other excluded-file patch is needed.
Main's concurrent edits have now applied the required field removal; observed
before starting the focused repository test. I did not modify that file.

## Implementation and recovery boundary

Removed NestedCursor::Xor.steps and the clone at route settlement. A private
selected_body lookup follows the selected name through captured parent steps.
current_body keeps its signature; invalid selection returns no executable body.
finish reports the missing captured parent/path without changing the cursor.
A child index beyond the captured body also fails without mutation; exactly
completed children still return to the parent without replay.

Serde ignores historical cursor `steps` fields. They cannot supply executable
content: current_body and finish require the enclosing captured definition.
Strict ConcreteXor/ConcretePath decoding remains owned by main. Older records
without authoritative captured definitions need main's explicit recovery
disposition; this change neither reloads sources nor claims those old child
copies preserve unrecorded alternatives. Once a parent is authoritative, even
a differing historical cursor copy cannot replace its definition.

## Focused proof

The nested saved-decision fixture now
round-trips captured definitions, inserts old cursor copies at two levels,
checks they disappear on serialization, deletes all local Flow/Skill sources,
and resumes the pending Iterate through captured work and the outer suffix.
A second fixture checks missing parent/path and out-of-range child position
leave the cursor unchanged and do not execute provider work.

**15 passed, 0 failed** in the real repository execution module. One focused
behavioral command was run, with ambient Home, Run, Task, Work, Wave, Flow and
human Session authority cleared:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_CONTROL_BIN \
  -u LF_HOME -u LF_DB_PATH -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  -u LF_TASK_ID -u LF_WORK_ID -u LF_WAVE_ID -u LF_FLOW_STEP \
  -u LF_HUMAN_SESSION -u LF_HUMAN_SESSION_RUN_BIND \
  cargo test -p loopflow --lib engine::execution::tests:: -- --nocapture
```

Build completed in 1m 15s including a shared Cargo lock wait; tests took 0.02s.
The existing RecordingExecutor simulates boundary side effects. No replacement
parser, harness crate, provider, database or production Home was used. Existing
coverage also passed for completed-child return, human-wait continuation,
empty branches, direction propagation, overlapping loops, and explicit blockers.
The updated SQLite test initializer compiled; its test was not selected.

File-scoped `rustfmt --check --edition 2021 --config skip_children=true` and
`git diff --check` passed for both changed Rust files. No all-target Clippy,
full-suite, live provider/human handoff, or historical unpinned-state recovery
pass is claimed. Main owns the latter recovery disposition and wider checks.
Read-only `lf top` observed machine activity during the build lock wait; no
process was signaled or changed.

Review: the lookup and settlement consume the same captured body; only the
cursor is cloned for atomic error handling. Exact boundary keys, route
candidates, direction propagation and iteration bookkeeping are unchanged.
Completed-child recovery still uses the existing parent return, not a second
interpreter. No alternate cursor, migration store or source lookup was added.

No commit, push, installation, rebase, Session mutation, or agent launch.
