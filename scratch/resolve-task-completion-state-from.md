# Resolve Task completion state from the issue's owning Linear team

## Problem

A Loopflow wave pins exactly one Linear team in `GOAL.md`
(`pm.linear_team`), but a Project can span issues from more than one team
(Developer Efficiency carries both `ENG-*` and `W2-*`). `complete_item` /
`reopen_item` resolve the target workflow state from the **wave-configured**
team, then hand Linear a state id belonging to a team the issue is not in.
Linear rejects the mutation: *"Discrepancy between issue team and state, cycle
or project."*

Worse, `lf pm task done` comments the PR link **before** closing, so a rejected
close leaves a "Shipped: …" comment on a still-open issue — a durable lie that
the work shipped when the issue never closed.

Who benefits: any Session whose issue lives in a team other than the wave's.
Right now those Sessions ship merged work and cannot record it; the Linear issue
is the only place the state can be made true, and that write is exactly what
breaks.

## The demo

```
$ lf pm task done --id <ENG issue> --pr https://github.com/.../pull/1035
commenting PR link on linear task <ENG issue>
completed linear task <ENG issue>
```

The `ENG-*` close now succeeds where it used to report
*"Discrepancy between issue team and state, cycle or project."*

## Approach

1. **Resolve the state from the issue's own team.** Add an `IssueTeam` query
   (`issue(id) { team { id } }`) and an `item_team_id(item_id)` helper.
   `complete_item` and `reopen_item` resolve the issue's owning team, then pick a
   workflow state from *that* team. `completed_state_id` takes a `team_id`
   parameter (mirroring `unstarted_state_id`) instead of calling
   `resolve_team_id` internally.

2. **Preserve creation-team behavior.** `create_item` / `create_project` keep
   using `resolve_team_id` (wave-configured team). New issues still land in the
   wave's team; only state transitions on *existing* issues read the issue's
   team.

3. **State before comment.** In `pm.rs`, transition the issue to Done **before**
   posting the "Shipped" comment. A failed close then leaves no comment behind;
   the comment follows the state.

## Key decisions

- **Read team per-transition, not cached.** One extra GraphQL round-trip per
  close/reopen. Completion is not hot; correctness beats saving a request.
- **No bulk reteam, no identifier change.** The fix reads the owning team; it
  never moves issues between teams (that path renumbers issues and is explicitly
  not the repair).
- **Reorder rather than compensate.** Doing state-then-comment is simpler than
  posting a comment and rolling it back on failure, and matches "the comment
  follows the state."

## Scope

- In: `complete_item`, `reopen_item`, `completed_state_id` signature, new
  `ISSUE_TEAM_QUERY` + DTO, `pm.rs` close ordering, tests.
- Out: bulk reteam, `pm reteam`, issue renumbering, `create_item` team logic.

## Done when

- `complete_item`/`reopen_item` resolve state from the issue's owning team.
- `lf pm task done` transitions state before commenting.
- Regression test: a client whose configured team differs from the issue's team
  completes the issue by reading the issue team. Sabotaged (state resolved from
  the wave team) it goes red.
- `cargo test`, `cargo fmt`, `cargo clippy -- -D warnings` pass.
- Reconcile ENG-26 / ENG-76 to Done after landing.
