# Assumptions — 2026-11-04

The Task brief selects bounded input-store retries while preserving drafts after
unexpected path loss. Historical request attribution is unavailable after the
owners exited. Fault injection proves recovery handling, not the incident's
cause; the moved-WAL counterexample and causal limits remain in
[the plan](loo-725.md#evidence-limits-and-remaining-acceptance).

Demo cleanup is no longer held open. Jack Heart's brief and LOO-722 comment
`2693d72d` exclude whole-root retention and draft-held roots, so the plan now
separates durable history from runtime and treats review's end as release. This is
a reading of his decisions, not a new decision by him; he can overturn it. See
[the plan](loo-725.md#demo-cleanup-decision--settled-by-accepted-decisions-2026-11-04).
An abandoned review releases nothing, and its root stays reported as retained.

This Task's `auto` graph has no demo step, and the assumption that LOO-722's demo would
carry the reading and the abandoned-review question failed: he approved it and its
record shows neither. They reach him through the PR body, which Auto-Ship can land before he
reads it; landing marks and deletes nothing. Installed acceptance
is assumed to belong to LOO-723's authorized rollout.
See [the owners](loo-725.md#obligations-and-owners--reconciled-2026-11-04).

The walkthrough found an edited draft stays `Uncertain` after it is sent, so a settled
conversation holds its owner until the provider exits. That is LOO-709's stated
contract and Jack Heart selected editable drafts, so it stays; the PR raises it, and
`demo.md` teaches the teardown. Clearing `completed_at` at owner bind is the simpler
choice over narrowing `NoOpenSession` in the watcher. The second concept review reads
Jack Heart's "`lf resume` always gets you back" as ending an unanswering owner of a
completed Session, since no work is known there; a draft nothing recorded can be lost,
as in idle takeover. See [the direction](loo-725.md#no-open-session-at-resume--concept-review-2026-11-04).

Marker guidance lives in `scripts/disk-cleanup.md`, and `.lf/skills/disk-cleanup.md` points
to it. A demo run does not load that skill, so `demo.md` carries one generic line sending it
to the repository's cleanup guidance; a demo agent must still find the file.

The cleanup proof ran under a private person Home, not the real `~/.lf-machine`: an unscoped
`Cleaner.run()` would reach other Tasks' roots. It proves the cleaner against real demo roots,
not the installed job. A surviving reparented provider is reported to LOO-722, not patched,
because its hooks proved bounded. See [the receipt](loo-725.md#cleanup-proof-receipt--2026-11-04).

`resume_takes_over_an_owner_from_another_build` failed "viewer left the conversation" in
2 of 9 parallel focused runs during the #1818 sync (load average 40-70) and 0 of 3 alone.
Its logic matches main's; the cause is unknown, and four diagnostic runs did not repeat it.
Gate or CI should show whether it recurs; a viewer leaving during takeover would be a
product bug, not only a test race.

Wave memory stayed read-only in the November 4 realign: no step of `auto`, `pursue`, `refresh`
or `deliver` calls for its upkeep. Earlier passes on this branch did edit it, including
condensing unrelated sections. Those edits stay and [the PR body](loo-725.md#delivery-text)
names them, because reverting would drop the LOO-725 section and is as easy later.

The November 4 fifth concept review treats abandoned-review age, durable-history expiry and
edited drafts outside demos as open but not this Graph's stopping condition: none changes this
diff, and each has a safe accepted default. Jack Heart's brief asks to carry `auto` through
delivery. If he wants them settled before landing, `lf ship --stop` holds the Task.
