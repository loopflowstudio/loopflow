# loop — loop a flow until its bit reads set

A **loop** is a looping flow. Any flow can loop — `task`, a scan
flow, a single skill. The flow's skills own everything about the work,
including how to decide it is done; the runner owns placement, the loop, and
caps. There are no tiers in code: "task" and "project" are just flows we
define.

```
lf loop task "fix the flaky chord-timeout test"  # inhabit until the PR merges
lf flow scan-pass "scan the runtime"             # run once in this worktree
lf --wave infra loop scan-pass "scan" --detach  # server owns a justified loop
```

A loop must allow at least two passes. Use `lf flow` when one pass is enough.

**The termination bit is one generic contract, identical for every
loop**, and the runner teaches it itself: every pass's seed carries a
standing `<lf:loop>` instruction explaining how to mark for termination
— so any flow is loopable without its skills knowing loop mechanics. The bit
is one file, read and removed at every boundary:

```yaml
# scratch/loop.yaml
done: true        # terminate the loop at this boundary
# or
recheck: gh pr view --json state -q .state | grep -q MERGED
```

`recheck` is an agent-authored predicate the runner polls mechanically for
free; when it exits 0 the loop runs one more pass so the flow can close out.
The mechanics come from the runner; the WHEN comes from the flow: the mutate
skills of purpose-built loop flows discuss it with context — for
`task`, the agent both drives the PR to merged and decides that
observing MERGED means flipping the bit. Self-report in a reply counts for
nothing; only the file does.

**Backlogs are allowed.** A task has three visible states: filed in Linear,
running as an `lf loop`, and merged as a PR. Waves read the filed backlog when
selecting; `lf runs` and the heartbeat's `<in_flight>` fold show the active
hands. Filing intent is legitimate, but never substitutes for selection.

Foreground and detached loops are one primitive. Foreground inhabitation
blocks the caller until the bit flips. `--detach` asks the live wave server to
own the loop in a named tmux session; attach with `tmux attach -r` for
read-only inspection. Both execute headlessly and fork a worktree. Detachment
is useful only when the parent has another move while the child runs; it is not
permission to create a loop.

## Layout

- `pass.rs` — one bounded, headless run of any flow in a worktree
  (`lf -b flow <flow>`, killed on timeout) — the loopable unit.
- `driver.rs` — the loop: place → pass → read the loop file → done / recheck
  / continue, under caps (max passes, wall clock; exhaustion escalates via
  `lf radio` on the hand's channel).
- `run.rs` — `LoopRun`: the registry-backed run lifecycle (worktree,
  store row, status). The row is what makes a running loop visible as an
  open task.
- `wave.rs` — the wave: the same pass, driven by the residency's event
  scheduler (inbox / heartbeat / cron) instead of a sequential loop, with no
  bit — the loop is the point. See `wave/README.md` for the topology.

The canonical flows live in `engine/builtins/build/`: `task`
(`task_clarify → task_pursue → task_mutate`, bit = merged PR) and
`project` (`project_clarify → project_pursue → project_mutate`, KR set
in the project's own doc, bit = all KRs checked). Tier behavior lives in the
skill texts — defining a new kind of loop is writing a flow + skills,
zero Rust. Those skills are inline-first: waves may create project/task loops,
projects may create task loops, and tasks never invoke `lf loop`. Operational
child commands such as `lf pr land` remain inline execution at every tier.

Phase runs are plumbing — never surfaced in the product. A served wave's
thread is the one conversation surface; bounded loops are hands, with only
their read-only tmux sessions exposed for inspection.
