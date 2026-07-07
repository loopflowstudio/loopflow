# flowloop — loop a flow until its bit reads set

A **flowloop** is a looping flow. Any flow can loop — `task-pass`, a scan
flow, a single skill. The flow's skills own everything about the work,
including how to decide it is done; the runner owns placement, the loop, and
caps. There are no tiers in code: "task" and "project" are just flows we
define.

```
lf task "fix the flaky chord-timeout test"     # loop task-pass to a merged PR
lf task "…" --flow scan-pass                   # loop any flow the same way
```

**The termination bit is one generic contract, identical for every
flowloop**, and the runner teaches it itself: every pass's seed carries a
standing `<lf:flowloop>` instruction explaining how to mark for termination
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
`task-pass`, the agent both drives the PR to merged and decides that
observing MERGED means flipping the bit. Self-report in a reply counts for
nothing; only the file does.

**No backlog.** A task exists in two states: a running `lf` run, and a
merged PR. The open runs with a task flow ARE the wave's open tasks
(`lf runs`, the wave heartbeat's `<in_flight>` fold); the PR is the record
of done. Intent that isn't running yet lives in GOAL.md, memory, and chat —
not in a tracker.

## Layout

- `pass.rs` — one bounded, headless run of any flow in a worktree
  (`lf -b <flow>`, killed on timeout) — the loopable unit.
- `driver.rs` — the loop: place → pass → read the loop file → done / recheck
  / continue, under caps (max passes, wall clock; exhaustion escalates via
  `lf chat --parent`).
- `run.rs` — `FlowloopRun`: the registry-backed run lifecycle (worktree,
  store row, status). The row is what makes a running loop visible as an
  open task.
- `wave.rs` — the wave: the same pass, driven by the residency's event
  scheduler (inbox / heartbeat / cron) instead of a sequential loop, with no
  bit — the loop is the point. See `wave/README.md` for the topology.

The canonical flows live in `engine/builtins/build/`: `task-pass`
(`task_clarify → task_pursue → task_mutate`, bit = merged PR) and
`project-pass` (`project_clarify → project_pursue → project_mutate`, KR set
in the project's own doc, bit = all KRs checked). Tier behavior lives in the
skill texts — defining a new kind of flowloop is writing a flow + skills,
zero Rust.

Phase runs are plumbing — never surfaced in the product. Chat is the one
interface to a flowloop; only execs surface as attachable sessions.
