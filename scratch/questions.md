# Open questions — intelligence wave

## 1. The daily/weekly cadence is not wired, but the KRs now assume it runs

`wave/intelligence/GOAL.md` frontmatter carries `crons: []`. The `## Cron`
section names a `daily` and a `weekly` audit, but `flowloop/wave.rs` reads only
the frontmatter `crons:` list, and no `daily` or `weekly` flow exists (`lf -l`).
So the prose is cadence intent for the wave mind to honor when it happens to
loop — nothing fires unattended. Product and infrastructure have the same shape,
so this reads as convention, not a bug local to this wave.

That was harmless while the KRs were qualitative. It is not harmless now: trace
and evals both carry endurance KRs that only a real schedule can satisfy —
"zero gap-days in a month", "runs weekly and on every release without manual
repair", "10/10 sampled runs replay unattended".

**Assumption taken, so the clarify pass could proceed:** the cadence is
aspirational, the KRs state the target end state, and wiring a real cron is
task-shaped work under `trace` (or a wave-level change if the convention should
change across all three waves). The KRs were written to the intended end state,
not to today's unscheduled reality.

Open: wire `crons:` to a real flow (which? `garden` and `govern-intelligence`
exist and are close in spirit), or keep the cadence as prose and accept that
the endurance KRs are measured by hand?

## 2. Linear is unreadable, so project selection is running blind

`lf pm show --wave intelligence` fails: *"Stored linear token has expired. Run
`lf auth linear` again."* This clarify pass therefore read only the local
project files, never the live tasks. If a task in Linear contradicts a KR here,
this pass did not see it. Needs `lf auth linear` before the next wave loop.
