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

## 3. manabot's missing runs were data loss, not non-adoption

An earlier reading of this said manabot had not been driven through loopflow
since 2026-05-18. That was wrong — it read prompt-log filenames, and an
`lf op shell init zsh` writes a journal but no prompt log.

`lf` ran in manabot four times on 2026-07-09 (`.lf/journal/runs/*/events.jsonl`,
15:30 UTC). The ledger has zero rows for all four run ids, because the ledger
was deaf: the `step_index`/`skill_index` drift broke `insert_run_event`, and
`ledger_insert` swallows the error into `debug!`.

| | |
|---|---|
| last ledger write | 2026-07-08 14:59:28 UTC |
| first write after the 055 repair | 2026-07-09 20:12:00 UTC |
| silent outage | **29.2 hours**, every repo on this machine |

Fixed forward: the first ledger failure per process now logs at `warn!`. The
schema drift was an accident; a best-effort write that whispers would have
hidden the next one too.

Standing question, now separable from the bug: hootro really is dormant (no
`.lf/journal`, nothing since 2026-06-19). Is it meant to be driven through
loopflow, or left as the control arm? Both are defensible.

## 4. Which corpus does the eval harvester target first?

The test-as-judge construction is validated (see `evals-design.md`), but the
environment tax decides where it can run today:

- **cadenza** `server/tests/*.py` — pure Python, validated in 0.5s. Ready.
- **loopflow** `rust/loopflow/tests/*.rs` — integration tests only; the inline
  `#[cfg(test)]` unit tests cannot be split from their code at file level.
- **manabot** — needs a `managym` native build pinned to the right Python ABI at
  the parent commit. Real work, deferred.
- **hootro** — unknown.

**Assumption taken:** harvest cadenza first, because it validated immediately
and it is the one non-loopflow repo already in the ledger. Slice 1 is the
harvester, not the runner — the corpus is the asset, and it costs no tokens.
