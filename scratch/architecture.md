# Current cut: land the durable control spine

[`docs/architecture.md`](../docs/architecture.md) remains the forward-looking
target. This file defines the smaller landing selected on 2026-07-18 and the
exact follow-up it creates.

## Moment of transparency

The normalized architecture is real and tested:

- `Work → Epoch → Run → Launch → optional Turn` is durable;
- one opaque `LF_RUN_LEASE` authorizes the exact active Run;
- Steer, Send, Basis, Wait, root Turn output, and Turn usage are authoritative;
- stored Review/Handoff and ChildCommand aggregates are deleted;
- Review route and pending attention are separate;
- a child reply re-arms attention once and advances parent evidence once;
- Wave and Project service direct input and oldest child Review before
  background work, with live or interrupt-and-seed delivery;
- user docs and built-in skills use the new Work/Launch surface.

Task and Project execution still runs through `ProjectSessionStatus`,
`TaskSessionStatus`, and `ChildWriteLease`. Those controllers mirror lifecycle
into Run. They are temporary implementation, not product concepts.

The 2026-07-18 gate is otherwise green but pins one incoherent bridge:
`task_github_cache_tests::rest_failure_opens_one_durable_circuit_while_local_controls_continue`
creates a live legacy Task body whose mirrored active Run has no product
Launch. Direct Run interrupt therefore cannot find a provider boundary.

## This landing

Make the normalized control spine independently correct while Session remains
the temporary Task/Project executor.

1. Every live legacy Task and Project body registers exactly one product
   `Launch` under its mirrored Run before it can receive control.
2. The Launch records the actual provider/process boundary and containment
   facts already known by the runner. Do not synthesize a process or infer one
   later from Session status.
3. Run interrupt, Steer delivery, monitoring, and recovery continue to query
   Launch. Do not add a fallback from Run to Session lookup.
4. Starting, retrying, replacing, or reaping a legacy body keeps Run and Launch
   lifecycle coherent. A stopped/revoked body cannot leave an apparently live
   Launch.
5. Keep Session as the sole Task/Project executor/write fence for this landing.
   Do not begin a partial second execution controller.
6. State this boundary plainly in architecture and PR copy. “Session” may
   remain internal execution vocabulary; it is no longer a public control API.
7. Rebase onto current main, preserve the already-executable migration tail
   without inventing a second draft registry, run the full gate, and prepare
   PR #1073 to land.

## Do not add

- Session fallback in `interrupt`, `steer`, or Launch lookup;
- a compatibility DTO that exposes Session as a core noun;
- a second Review, Handoff, Message, Command, or inbox aggregate;
- caller-supplied Run/Work/Author identity when the Run lease already proves it;
- a partial Task-only Run executor beside the legacy Project executor;
- a new migration ledger to imitate the documented but nonexistent `DRAFTS`
  runtime registry.

## Done when this PR can land

- every live Task/Project body has a product Launch before control is accepted;
- direct Run interrupt finds that boundary without consulting Session;
- Launch terminal state follows the body process honestly;
- stale/stopped Run leases still fail closed;
- the Review handshake, Project/Wave priority, exact lease, CI incident, and
  successor tests remain green;
- retired commands have zero user-doc, builtin-skill, and E2E references;
- `uv run python scripts/test.py --all` passes completely;
- copied-database migration, fmt, clippy, Rust, Swift, Python, website, E2E,
  and Mac build gates pass;
- the branch is rebased, the PR describes this landing rather than claiming
  Session deletion, and CI is green.

The 121,819 Rust-line ceiling and zero Session-controller references move to
the follow-up. This PR may not meet them by deleting behavioral proof.

## Follow-up Task: delete Session control

The follow-up is one connected, one-way cut. It begins from landed main and
owns all remaining duplicate execution authority:

1. Make `task/runner.rs` execute through shared Run `reserve | advance | stop`.
2. Make `project_session/runner.rs` use the same controller.
3. Move keeper recovery and containment release through Run. Only proven
   `Absent` releases the active slot; `Present` and `Unprovable` remain fenced.
4. Collapse Task/Project control DTOs and commands onto Work/Run controls.
5. Delete `ProjectSessionStatus`, `TaskSessionStatus`, `ChildWriteLease`,
   `ChildLeaseState`, body generations, Session authority env vars, duplicate
   lifecycle runners, and their store readers/writers.
6. Preserve only stable Project/Task Work facts and private provider
   continuation data on Launch.
7. Consolidate the final schema and migration boundary after every Session
   writer is gone.

Follow-up acceptance:

- Wave, Project, and Task share the same Run transition suite;
- no Session/body generation is needed to locate Work or authorize mutation;
- `rg 'ProjectSessionStatus|TaskSessionStatus|ChildWriteLease'` has zero
  production controller references;
- no legacy Session authority environment variables remain;
- Run containment and keeper recovery pass deterministic race tests;
- a parked Review preempts at most once and current CI claim cannot overlap a
  repair Run;
- Rust source is at or below 121,819 Tokei code lines on the landing base;
- full gate and migration tests pass with one live implementation.

Provider-native conversation/session ids remain private Launch continuation
data. User-openable tmux/TUI processes remain Launches. The noun being deleted
is Loopflow's duplicate Task/Project Session controller.
