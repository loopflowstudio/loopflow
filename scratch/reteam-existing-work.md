# W2-155 PR 2/2 — `lf pm reteam`: migrate existing W2 issues to per-wave teams

PR 1 (#899, merged) delivered the forward-cut: each wave binds `pm.linear_team`,
`lf pm init --team-key` adopts/creates the team, `build_client` resolves the
wave team, and `lf pm doctor` flags missing/shared bindings. New work already
gets `PRD-*` / `INF-*` / `INT-*`. This PR moves the **existing** open, settled
`W2-*` issues into their wave's team.

## The load-bearing constraint (settled, do not relitigate)

Linear **reassigns the issue number** on a team move: `W2-155` → `PRD-<next in
PRD>`, never `PRD-155`. So this is **not** a `W2-N → PRD-N` rename and must never
be described or built as one. What we preserve is **stable identity + a
traceable link**, not the number:

- Issue **UUID** is preserved by the move → every Task/Project Session (keyed by
  UUID via `get_task_session_by_issue`), PR link, comment, relationship, and
  ordering survives untouched.
- Traceability is a posted comment recording the prior identifier (`was
  W2-155`), because the number itself cannot be carried.

## What we move, defer, and leave

Candidate set = open issues under the wave's Initiative Projects
(`list_projects` → `list_items`; `PmItem { id: UUID, identifier, completed }`).

- **Leave (historical):** `completed == true`. Completed `W2-N` is referenced by
  shipped PR titles / commits / MEMORY — immutable records. Moving would renumber
  and orphan those references for zero gain. Policy documented in command help +
  README.
- **Defer (protect active/in-review):** look up `get_task_session_by_issue(uuid)`;
  any **non-terminal** Session defers. `TaskSessionStatus::is_terminal()` is
  `Completed | Abandoned` only, so Created/Starting/Running/Waiting/Blocked/Failed
  all defer — the conservative rule the directive wants ("defer every Task with an
  active or in-review Session"). This is the W2-155-moves-itself hazard: W2-155's
  own Session is Running, so the migration **defers itself** by construction.
- **Move:** open, not completed, no Session or terminal Session, not already in
  the target team.

## Idempotency / restart-safety (no schema change)

The target team's **key** is known when we resolve the wave team. An issue
already migrated carries identifier `KEY-N`; one still in the old team carries
`W2-N`. **Skip any item whose `identifier` already starts with `<KEY>-`.** That
is the idempotency key — no local ledger, no `PmItem.team_id` field (avoids a
speculative DTO field), restart-safe: a re-run re-lists, sees the moved issues
already prefixed, and performs no duplicate move. Re-running `pm init` is already
idempotent from PR 1, so setup+migration re-run is a full no-op.

## New building blocks (all others already exist)

1. **`LinearClient::move_item_to_team(item_id, team_id) -> PmResult<String>`** —
   one `issueUpdate(input: { teamId })` mutation that **selects `identifier` in
   the response** and returns the new identifier. Model exactly on
   `move_item_to_project` (linear.rs:623); the `issueUpdate` shape at 152/161
   already exists. Add `MOVE_ITEM_TO_TEAM_MUTATION` with an `identifier` field in
   the selection set.
2. **`lf pm reteam [--wave <w>] [--apply]`** — new `PmCommand::Reteam`. **Dry-run
   is the default**; `--apply` executes. Reuses `resolve_context`/`build_client`.
3. **`ops::pm::pm_reteam(...)`** — resolves the wave team (id + key), lists
   candidates, classifies each into move / defer / skip-completed / skip-already,
   and on `--apply`: `move_item_to_team` → `client.comment(uuid, "was W2-…")` →
   one `pm sync` at the end to refresh the snapshot's cached identifiers.
4. **Doctor stranded-issue check** — extend `pm_sync(plan:true)` (PR 1 already
   added team-binding/shared-team checks) to flag open, non-completed, no-active-
   session issues still under the old team for a wave that now has a `pm.linear_team`
   — i.e. `reteam` candidates not yet moved.

## Dry-run output (the Proof's "names every … ")

For the wave, print three lists + the target team:
- **Will move** — `W2-N "title"` each, with the caveat that the new number is
  Linear-assigned at move time (not predictable).
- **Will defer** — `W2-N "title"` + reason (`Session running` / `submitted` /
  `blocked` …), one line each.
- **Will leave (historical)** — completed `W2-N` count (policy line, not per-issue
  spam).
No mutation happens without `--apply`.

## Consistency (no Swift schema change)

The Mac reads identifiers straight from the snapshot (`BacklogItem`,
`WaveDetailPane`); the final `pm sync` propagates new prefixes with no Swift/DTO
change. Only add a team/prefix DTO field if a surface must *show* the team —
not required by the Proof; skip (DTO rule).

## Verification target (for pursue)

- **Unit** (mock Linear via the existing `pm::test_server` harness in linear.rs):
  - `move_item_to_team` issues `issueUpdate(teamId)` and returns the new
    identifier from the response.
  - `pm_reteam` dry-run classifies: a completed issue → leave; an issue with a
    Running Session → defer; an already-`KEY-`-prefixed issue → skip; a plain
    open issue → move. Assert the printed plan.
  - Second run after a simulated move (items already `KEY-` prefixed) → zero
    moves (idempotent).
- **Live proof** (real Linear, bounded): `lf pm reteam --wave product` dry-run
  names movable issues + defers W2-155 itself (Running) + leaves completed as
  historical. A single real `--apply` on one safe settled issue renumbers it,
  posts `was W2-…`, Session lookup by UUID still resolves, and `pm sync` +
  `pm doctor` finish clean.

## Serial-PR note

This is the final PR for W2-155. Land with `lf pr land -c` once the migration is
proven — the merge completes the Task. `reteam` deferring W2-155's own issue is
intended (it can only be moved later from a clean context), and does not block
Task completion: the *mechanism* is proven, the self-move is correctly deferred.
