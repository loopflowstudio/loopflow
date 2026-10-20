# LOO-523 open questions (decided provisionally; Jack reviews before landing)

1. **Superseded by Jack Heart (2026-10-20):** the committed `machines:` block is
   deleted; each person's Wave Machine is Machine-local (`lf wave machine`).
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

## Wave memory

Applied during compress (2026-10-20): infrastructure MEMORY.md drops placement,
admission and Started claims and records LOO-523 in the LOO-412 section.

14. **The ownership verbs are `claim`, `handoff` and `unclaim`** (Jack Heart, 2026-10-20).
    `lf task release` made `lf release` ambiguous; `unclaim` pairs with `claim` and
    keeps the shorthand.

## Jack Heart's direction (2026-10-20): the Wave Machine is personal

- Everyone in a repository has a repo Session; everyone in a Wave has a Wave
  Session for it. Which Machine runs those Sessions is that person's choice, not
  shared repository content: the committed `.lf/config.yaml` `machines:` block goes.
- **Wave Machine default:** a person can set the Machine their Wave's Tasks run on
  (laptop + mini: some Waves run on mini). A Task about that Wave starts there by
  default, including when launched from the laptop. Both Machines know the
  arrangement; the mini only needs to receive the runs.
- **Captured, not inherited:** like a Workflow or Flow, a Task keeps the Machine it
  started on. Changing the Wave default later does not move existing Tasks, and a
  Task may move to another Machine.
- **Visibility:** the Machine running a claimed Task is public by default (another
  person can see it from the Linear Task), but deprioritized after the person.
  Making it configurable is not essential. Audit anything that makes the Machine
  *have* to be public; nothing another person does should depend on it.
- **Decided by Jack Heart (2026-10-20): forwarding stays inside one person's
  Machines.** Your laptop routes ownership commands to your mini. Another person
  never routes to your Machine; their claim is written on their own Machine and the
  later claim wins. The Machine in the Linear claim comment is informational only.
- **Decided by Jack Heart (2026-10-20): `lf wave machine <wave> <machine>`** stores
  the person's Wave Machine default on this Machine and copies it to their other
  registered Machines over `lf --machine`. Setting it also moves that person's Wave
  Session to the Machine, restarting it there if it does not already live there.
- **Repo Session Machine (open):** Wave Sessions follow a personal Wave Machine;
  the repo Session stays on whichever Machine runs it. A personal repo Machine
  (`lf repo machine`) would mirror the Wave setting; deferred until wanted.
- **Moving a Session leaves its native history behind** on the old Machine; the
  successor starts with fresh Wave context, like `--fresh`.

## Implementation choices (2026-10-20, needs Jack's review)

15. **`wave_machines` is keyed by Wave id alone.** A Machine's store belongs to one
    person, so "the person's" Wave Machine needs no person column. Copies travel by
    Wave name with `--repository`, since Wave ids are per Machine.
16. **Setting the Wave Machine admits, not launches, the Wave Session there.** The
    target admits a primary (`ensure`, as `--prepare` does); the provider starts on
    the next connect. Other Machines complete their Wave primary. `--unset` moves no
    Session: every Machine then runs its own.
17. **Recorded entries (no person) count as yours**, and so does every entry when
    this Machine has no participant name; single-person use needs no name.
18. **Deferred from "primary Sessions follow the Machine":** context-resolved
    connects (no `--wave`) routing to the Wave Machine, `observe_task_location`
    reading the owner Machine, and `lf open` reading remote primaries. Explicit
    `--wave`/`--task` connects already route.
