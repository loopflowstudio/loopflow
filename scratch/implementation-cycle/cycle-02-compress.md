# Cycle 2 — compress: named native Sessions after continuation integration

Base: `0a4731f9d` (integration commit). This compress pass changes two Rust files
and leaves them uncommitted. Patch: `/tmp/loo291-c2-compress.patch`.

## Model before

Session lookup had two resolvers with overlapping enums:

- `SessionTarget { Interactive{dir, manifest, provider_session}, Ask, Flow }`
  from `find_session`, used by open/complete/completion_worktree. Standalone
  Flow Sessions were handled separately at the top of each operation
  (`flow_session::parse_id` then `attribute_standalone_session`).
- `NamedSession { Interactive{dir, manifest}, Boundary(Box<SessionTarget>),
  StandaloneFlow(StepToken) }` from `find_named_session`, used only by rename.
  It resolved Run IDs to their boundary; `find_session` did not.

So `$LF_RUN_ID` for an Ask or Flow review resolved to the boundary under rename,
but to a plain interactive Session under open and complete. Completing that way
would resolve the provider history without completing the Ask or review.
`find_named_session` also built full surfaces for every standalone Flow just to
match a Run ID. `boundary_run_ids` built the same surfaces, then reread each
position file.

## Model after

One `SessionTarget { Interactive{dir, manifest}, Ask, Flow, StandaloneFlow(StepToken) }`
and one `find_session`, which resolves in this order: standalone id, then a Run
ID linked to a waiting boundary (standalone, Ask, managed Flow), then an
interactive Run, then an Ask id, then a managed Flow id. `session_surface`
projects all four kinds. The standalone enrichment (`attribute_standalone_session`)
is now called from two places, `list` and `session_surface`, down from five.

- open, complete, completion_worktree and rename all go through `find_session`.
  `boundary_id(&target)` gives the durable id to lock and prepare, whatever id
  the caller passed.
- Interactive provider history is read where it is used: `provider_history` in
  open (for resume) and complete (this keeps their existing "no provider history
  yet" refusal). Rename doesn't need history, as before.
- `flow_session::reviews()` returns `(FlowRun, StepToken)` for each waiting human
  review. `list`, Run-ID boundary lookup and `boundary_run_ids` share it, so none
  of them builds surfaces just to read a Run ID.

Deleted: `NamedSession`, `named_surface`, `find_named_session`, the three
`parse_id` pre-checks in open/complete/completion_worktree, and the standalone
reread loop in `boundary_run_ids`.

### Behavior changes (intentional, small)

- A boundary's Run ID now targets its boundary for open and complete, as it
  already did for rename. `list` never exposes those Run IDs as Session ids, so
  the Mac is unaffected.
- Standalone complete now also checks the shared `require_session_action`
  policy. A review that isn't ready gets the shared "not marked ready" message,
  where it used to get flow_session's own ready check (which still runs).

## Deliberately retained

- `attribute_standalone_session`: `flow_session` is synchronous and store-free,
  and moving Work attribution there would spread async store access into Flow
  file ownership. Projecting in one place in `session_surface` is enough.
- `session_run_id`, `prepare_boundary`, `carry_session_name`, `recover_unpublished_run`:
  each is prelaunch Run identity or preserves a human name across replacement.
  None of them duplicates another.
- The remote-Home rename refusal only applies to managed Flow. Standalone
  reviews are local files, and Ask is local.
- The cursor and FlowPosition ownership are unchanged. Human reviews stay
  Complete-only; loop-decide owns Advance and Iterate.
- Swift `SessionRecord` field-by-field matches Rust (`run_id`, `title_source`
  including `unavailable`, `flow_membership`, actions Open/MoveHere/Complete).
  No Swift change was needed. No native control policy was duplicated.
- The `engine/agent.rs` Claude stdin transport was reviewed. Batch and streaming
  launch only for `process.auto`, so moving `stdin(null)` into the launch branch
  preserves non-Claude and interactive behavior. A fresh anonymous file is made
  per attempt. No change.
- Feature topology, prototype files and the diagram were not touched.

## Proof

Environment Home/Run/Flow/Session variables were unset.

- `cargo test -p loopflow --lib -- ops::human_session::tests ops::flow_session::tests flow_sessions_are_named_through_their_run_and_keep_exact_membership --test-threads=1`: 21 passed.
- `cargo test -p loopflow --test session_cli_tests`: 7 passed. These cover
  Run-ID rename inside an Ask before provider history, name carry-over on
  replacement, prelaunch identity, and both initial and resumed native clients.
- `cargo test -p loopflow --test flow_tests bound_flows_keep_task_context_and_leave_managed_flow_and_shared_edits_alone`:
  1 passed. It covers rename of a standalone review by its Run ID, and JSON
  reopen keeping the same boundary, Run and name.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all --check` both
  pass.

Log: `/tmp/loo291-c2-compress-tests.log`, `/tmp/loo291-c2-compress-clippy.log`.
The first test attempt did not run: a zsh `env -u` word-splitting mistake.
No Swift, native or configured proof was rerun (no Swift change).

Source SHA-256:
- `rust/loopflow/src/ops/human_session.rs` e37a66b5a86237de8d922fece394af041e2d31f71f6d24c0c13e3e748028e79d
- `rust/loopflow/src/ops/flow_session.rs` 86c077e4257646dc7533172f27b312147f2b4d711502257495d66851aba268cb

## Gaps for review

- No test yet completes a boundary by its Run ID (the new open/complete
  resolution). Rename by Run ID is covered.
- The remote-Home rename refusal is still covered by source inspection only.
- The Codex rejected-turn response defect is out of scope and was not touched.
- No commit, publication, PM action, install or live Session action was taken.
