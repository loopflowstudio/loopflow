# LOO-523 open questions (decided provisionally; Jack reviews before landing)

1. **Owning Machines live in `.lf/config.yaml`** (`machines.default`, `machines.waves.<name>`).
   This replaces the journal-held Wave placement, so the setting travels in Linear
   mode too. `lf wave place` is deleted rather than taught to edit YAML. The
   alternative is to keep a CLI that rewrites config.
2. **Decided by Jack Heart (2026-10-19): an unconfigured repository resolves to
   "this Machine".** Both claims push to the provider, and the second claim wins.
   Machines that never connect through a provider do not share the Task. The
   losing worker is not cancelled and stays the owner on its own Machine. It
   learns of the later claim from status and its context block, then decides.
   Its local claim ends only through its own unclaim or handoff.
3. **A claim moves Linear to `started`; an unclaim clears the assignee and leaves
   the state alone.** The alternative is for unclaim to queue `unstarted`.
4. **Completion writes no unclaim.** A Done Task shows its last owner as history.
5. **Decided by Jack Heart (2026-10-19): Linear can never push an unassignment
   to a Machine that thinks it owns the Task.** A Linear assignee edit creates no
   ownership entry. When a claiming Machine sees that Linear no longer shows
   it as assignee, it tells Linear again by re-queueing its assignee. It does
   this only while it has seen no later claim. Git works the same way: a
   foreign `unclaim` or `handoff` of a local open claim makes the owner append
   a fresh claim.
6. **Cross-person handoff is deferred.** Unclaim followed by the other person's
   claim covers it for now.

## Implementation choices (2026-10-19, needs Jack's review)

7. **Git transport rides the same marked comment as Linear**, not a dedicated
   `WorkValue::TaskOwnership`. The comment is already a journal record, so one
   transport covers both modes and the planning writer needs no new record kind.
   Git order is the comment's time, which each local append sets after every entry
   that Machine has seen (Lamport-style), so clock skew cannot reorder a re-claim
   behind a stale entry.
8. **One mutable column on an append-only row.** `task_ownership.ordered_at` may be
   stamped once (NULL → value) when the provider reports its order; every other
   column is immutable by trigger. Alternative: a separate order table.
9. **No viewer-id cache.** The Linear viewer is queried when a claim needs the
   assignee (claim, run, re-assert). Claims are rare; a cache adds state.
10. **The "also claimed later" notice is a Task steer**, so it reaches live Sessions
    through the existing steer delivery instead of a second context-block channel.
    It is written once, when the later claim first arrives.
11. **Desktop shows the owner line and Owned/Unowned grouping; the full log is in
    `lf task status`.** Listing the log in Desktop Task detail needs the log in a
    Desktop-read DTO; deferred.
12. **`lf wave place` is deleted; `lf wave rename` moved to its own module.**

## Proposed Wave memory update (infrastructure MEMORY.md is read-only here)

- Line ~300 says Machine removal keeps "identity, route and placements". Placements
  no longer exist; proposed: "identity and route survive, and ownership entries that
  name the Machine keep it".
- Add: "LOO-523 (2026-10-19): Tasks are owned by (person, Machine) through an
  append-only `task_ownership` log; Started, Work placement and first-start
  admission are deleted. Owning Machines live in `.lf/config.yaml` `machines:`."
13. **Renaming a Wave does not rewrite `machines.waves.<name>` in `.lf/config.yaml`.**
    The entry is keyed by name, so a rename leaves it pointing at the old name until
    someone edits config. Alternative: key by Wave id (less readable).
14. **The ownership verbs are `claim`, `handoff` and `unclaim`** (Jack Heart, 2026-10-20).
    `lf task release` made `lf release` ambiguous; `unclaim` pairs with `claim` and
    keeps the shorthand.
