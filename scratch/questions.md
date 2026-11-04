# Assumptions — 2026-11-04

The Task brief selects bounded input-store retries while preserving drafts after
unexpected path loss. Historical request attribution is unavailable after the
owners exited. Fault injection proves recovery handling, not the incident's
cause; the moved-WAL counterexample and causal limits remain in
[the plan](loo-725.md#evidence-limits-and-remaining-acceptance).

Demo cleanup is no longer held open. Jack Heart's brief and LOO-722 comment
`2693d72d` exclude whole-root retention and draft-held roots, so the plan now
separates durable history from runtime and treats review's end as release. This is
a reading of his decisions, not a new decision by him; his demo review can overturn
it. See [the plan](loo-725.md#demo-cleanup-decision--settled-by-accepted-decisions-2026-11-04).
An abandoned review releases nothing, and its root stays reported as retained.

Marker guidance lives in `.lf/skills/disk-cleanup.md`, the only live cleanup guidance
under `.lf/`. A demo run does not load that skill, so `demo.md` carries one generic
line sending it to the repository's cleanup guidance; a demo agent must still find the
file. LOO-722 rewrites the same skill, so that text conflict needs this section kept.
