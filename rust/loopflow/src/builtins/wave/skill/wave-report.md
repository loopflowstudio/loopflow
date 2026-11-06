---
requires: .lf/wave/
produces: scratch/wave-report.md
---
Read health signals across all waves. Surface where the system is stressed.

The report is a triage, not a status update: someone opens it and knows where
to spend their next hour — which waves need thinking, which can run on their
own, and which are failing quietly. Read `scratch/` first.

## Signals

### 1. Staleness

Time since the wave last moved: commits on main in its area, PRs opened or
merged, items completed.

Gather `git log main --since="2 weeks ago"` per wave area, open and recently
merged PRs touching it, and the age of the latest item status change.

- Silence in a dormant wave (`mode: manual`, no active items) is healthy. One
  line and move on.
- Silence with active items or open PRs is a stall. How long, and what happened
  last?
- Activity that suddenly stops is sharper than chronic quiet: 5 PRs last week
  and none this week is different from a month of quiet.

### 2. Dependency pressure

Which waves are load-bearing, and which are leaves? Items declare what they wait on:

```markdown
# 4: Calibration View

**Needs:** model/3-wave-modes, model/4-letta-integration
```

`needs:` is item-to-item; "macos is blocked on model" is shorthand for it.
People write it up front, build agents add it when they hit a wall, and Wave or
repository passes add it when they keep rediscovering the same blocker.

Gather `**Needs:**` lines (the main signal), trigger declarations
(`signal: wave, source_wave_id: X` makes X upstream), overlapping area paths,
and item text naming other waves.

- For each `needs:`, check whether the needed item shipped. Count the blocked
  items across all waves that wait on *this* wave: that's its pressure. Six
  items in three waves waiting on one wave is real pressure.
- Overlapping areas with no trigger or `needs:` is uncoordinated coupling —
  merge conflict territory.
- When you find an undeclared dependency, propose adding `needs:` to the item.

### 3. Design debt

Signs the wave pushed forward and got pushed back.

Gather closed-unmerged PRs, branches ahead of main with no PR, `scratch/` notes
on abandoned approaches, item files heavily rewritten (git log), and items moved
between priority tiers.

- One abandoned PR is iteration. Three on the same wave means the direction is
  unclear: it needs design thinking, not more build cycles.
- A rich plan replaced by a smaller one is either simplification or retreat.
  Check whether the replacement shipped.
- Items rewritten again and again without shipping are the strongest signal:
  the wave doesn't know what it wants.

### 4. Velocity mismatch

Activity against backlog shows whether a wave is converging or thrashing.

Gather commits in the area over 2 weeks, items per priority tier (1=now through
4=later), items completed in the same window, and PR open-to-merge time.

- High commits, shrinking backlog: converging.
- High commits, flat or growing backlog: thrashing. Items may be too big or
  badly scoped.
- Low commits, flat backlog: dormant. Fine if intended, a stall if not.
- PRs that land in minutes may be shallow; check the size of the changes.

### 5. Coherence

Do the wave's items pull in one direction, or is it a dumping ground?

Read the Wave's current chapter, key results and Tasks, plus `GOAL.md` and
`MEMORY.md`.

- Chapter outcomes and Tasks should serve the Wave's objective. A Task in the
  `pm` wave that's really Loopflow UI belongs in `macos`.
- Tasks at one priority tier should run in parallel. If 2a blocks 2b, report
  the sequencing.
- Items spanning four subsystems suggest a split: one wave, one concern.
- Items older than the last major redesign may be stale. Check the finish line
  still makes sense.

## Workflow

1. **Enumerate waves.** Read each `.lf/wave/*/`: YAML config, README, item
   files. Skip `.lf/wave/old/`.
2. **Gather all five signals** with git, gh and file reads before interpreting
   any of them.
3. **Score each wave on each signal:**

   | Level | Meaning |
   |-------|---------|
   | quiet | No signal. Healthy silence or doesn't apply. |
   | warm | Worth noting. Not urgent. |
   | hot | Needs attention now. Gets worse if ignored. |

4. **Write the report** by signal intensity, hottest first. A wave quiet on all
   five gets one line.

## Task references

For more than one Task, read `lf wave show <wave> --json` for plan rows and
`lf wave status <wave> --json` for live execution. Render lists as:

```markdown
[identifier · Task title](provider URL) — status; next action/owner
```

In prose, use `[Task title · identifier](provider URL)` on first mention and
shorten later.

Fill the link from `task.identifier` and `reference.issue_url`, the title from
`task.name`, status from `runtime.status` or the roadmap `section`, and next
owner from `next_move.owner`. State a next action only when current status
supports it; leave unknowns unknown. Include `reference.workspace.slug` only
when opening that workspace is the job.
A Task can have a checkout without a PR. Never guess a URL, branch or slug from
a title or identifier; without a link, keep the title and status.

## Output

Write `scratch/wave-report.md`:

```markdown
# Wave Report — <date>

## Attention needed

<Waves with hot signals: which signals, the specifics, and what kind of
attention — design thinking, unblocking, descoping, splitting, or just landing
existing work>

## Warming

<Waves with warm signals. What to watch.>

## Quiet

<One line each.>

## Signal map

| Wave | Staleness | Dep Pressure | Design Debt | Velocity | Coherence |
|------|-----------|--------------|-------------|----------|-----------|
| ...  | quiet/warm/hot | ... | ... | ... | ... |

## Raw observations

<Cross-wave patterns, surprises, anything that doesn't fit the five signals.>
```

## Rules

- Depth follows heat. A quiet wave gets a line; a hot one gets a paragraph.
- Commits and PRs are inputs, not outcomes. A correctly dormant wave is
  healthier than one with 50 commits and no finish lines.
- Diagnose, don't prescribe. "This wave needs design thinking" belongs here;
  "split item 3 and rewrite the README" doesn't.
- Three levels are enough. No 1–10 scales or weighted formulas.
