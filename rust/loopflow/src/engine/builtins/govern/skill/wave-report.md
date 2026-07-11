---
requires: wave/
produces: scratch/wave-report.md
---
Read health signals across all waves. Surface where the system is stressed.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Cells don't ask each other "how are you doing?" They sense chemical
gradients, electrical impulses, mechanical pressure. The brain integrates
millions of these signals into a felt sense of what needs attention.

This step reads five signal types across all waves and produces a
health report. The report tells a human which waves need their thinking,
which can run autonomously, and which are silently failing.

Success: a human reads the report and knows where to spend their next
hour. Not a status update — a triage.

## Signals

### 1. Staleness

Time since the wave last moved. Movement means commits landing on main,
PRs opened or merged, or Linear tasks completed.

Gather:
- `git log main --since="2 weeks ago"` filtered to each wave's area paths
- Open and recently merged PRs touching wave areas
- Open and completed task counts from `lf pm show --wave <wave> --json --no-sync`
- PM snapshot `synced_at`; stale planning data is its own warning

Interpret:
- Silence in a wave with no open tasks or PRs is healthy.
  Note it and move on.
- Silence in a wave with open tasks or PRs is a stall signal.
  How long has it been quiet? What was the last thing that happened?
- Recent activity that suddenly stops is sharper than chronic inactivity.
  A wave that shipped 5 PRs last week and nothing this week is different
  from one that's been quiet for a month.

### 2. Dependency pressure

Which waves are load-bearing? Which are leaves?

Each Linear task can declare what it is waiting on in its description:

```markdown
**Needs:** model/TASK-123, product/TASK-456
```

`Needs:` is task-to-task. "macos is blocked on model" is shorthand for
specific tasks in macos needing specific tasks in model. The wave-level
roll-up is useful for the report, but the actual dependency lives
between items.

Three sources write `needs:`:
- Humans, when they know the dependency up front
- Build agents, when they hit a wall during implementation
- The garden, when it keeps rediscovering the same blocking pattern

Gather:
- `**Needs:**` declarations in task descriptions — the primary signal
- Trigger declarations: `signal: wave, source_wave_id: X` means X is
  upstream of the declaring wave
- Area overlap: waves whose area paths intersect have implicit coupling
- Task text that references other waves by name

Interpret:
- For each task with `Needs:`, check whether the needed task has
  shipped. If not, the task is blocked. Roll up: how many tasks across
  all waves are blocked on tasks in *this* wave? That's the pressure
  score.
- A wave where 6 items across 3 other waves need its deliverables is
  under more pressure than one with zero downstream blockers.
- Area overlap without explicit triggers or `needs:` means
  uncoordinated coupling — potential merge conflict territory.
- When the garden discovers a dependency that isn't declared, propose
  adding `needs:` to the item. Sensed first, declared when confirmed.

### 3. Design debt

Signals that the wave tried to move forward and got pushed back.
Failed attempts leave traces.

Gather:
- Closed PRs without merge (abandoned work)
- Branches ahead of main with no open PR (started but not finished)
- `scratch/` artifacts that reference descoped or abandoned approaches
- Repeated replacement tasks or abandoned implementations around the same KR

Interpret:
- One abandoned PR is normal iteration. Three abandoned PRs on the same
  wave means the direction is unclear — the wave needs design thinking,
  not more build cycles.
- Descoped designs (a rich plan replaced by a smaller one) can be
  healthy simplification or a sign of retreat. Look at whether the
  replacement shipped or also stalled.
- Tasks that keep getting replaced without shipping are the strongest
  signal. The wave doesn't know what it wants.

### 4. Velocity mismatch

The relationship between activity and backlog tells you whether a wave
is converging or thrashing.

Gather:
- Commit count in wave area over last 2 weeks
- Open and completed task counts in the current PM snapshot
- PR cycle time: opened → merged duration

Interpret:
- High commits + shrinking backlog = healthy convergence. The wave is
  finishing things.
- High commits + stable or growing backlog = thrashing. Work is
  happening but finish lines aren't being crossed. Items may be too
  large or poorly scoped.
- Low commits + stable backlog = dormant. Fine if intentional, a stall
  if not (see staleness).
- Very fast PR cycle times can mask shallow work — PRs that land in
  minutes may not be substantive. Cross-reference with the depth of
  changes.

### 5. Coherence

Are the projects and tasks in a wave pulling in the same direction, or has the wave
become a dumping ground?

Gather:
- Read every Project and task in `lf pm show --wave <wave> --json --no-sync`
- Read the wave `GOAL.md` and `MEMORY.md`
- Check code-area overlap between open tasks within the same wave

Interpret:
- Projects and tasks should serve the wave's stated objective. A task in the `pm` wave
  that's really about Loopflow UI belongs in `macos`.
- Projects should be independent enough to work in parallel. If one task
  blocks another, that's a sequencing issue the
  report should surface.
- A wave with items spanning 4 different subsystems may need to be split.
  One wave = one coherent concern.
- Tasks that predate the last major redesign
  may be stale. Check whether the finish line still makes sense given
  what's shipped since.

## Workflow

1. **Enumerate waves.** Read `wave/*/` directories. For each, load `GOAL.md`,
   `MEMORY.md`, and `lf pm show --wave <wave> --json --no-sync`. Skip `wave/old/`.

2. **Gather signals.** For each of the five signal types, run the
   gathering steps described above. Use git, gh, and file reads.
   Gather everything before interpreting anything.

3. **Score each wave on each signal.** Use a three-level scale:

   | Level | Meaning |
   |-------|---------|
   | quiet | No signal. Healthy silence or irrelevant dimension. |
   | warm | Something worth noting. Not urgent. |
   | hot | Needs human attention. Will get worse if ignored. |

4. **Write the report.** Organize by signal intensity, not by wave.
   Hot signals first. A wave that's quiet on all five dimensions gets
   one line, not a section.

## Output

Write `scratch/wave-report.md`:

```markdown
# Wave Report — <date>

## Attention needed

<Waves with hot signals. For each: which signals are hot, the specific
evidence, and what kind of attention is needed (design thinking,
unblocking, descoping, splitting, or just landing existing work)>

## Warming

<Waves with warm signals. Brief notes on what to watch.>

## Quiet

<Waves with no active signals. One line each.>

## Signal map

| Wave | Staleness | Dep Pressure | Design Debt | Velocity | Coherence |
|------|-----------|--------------|-------------|----------|-----------|
| ...  | quiet/warm/hot | ... | ... | ... | ... |

## Raw observations

<Anything that doesn't fit the five signals but seems important.
Cross-wave patterns, surprises, anomalies.>
```

## Anti-patterns

**Uniform depth.** Don't write the same amount about every wave. A quiet
wave gets one line. A hot wave gets a paragraph. Match output depth to
signal intensity.

**Activity worship.** Commits and PRs are inputs, not outcomes. A wave
with zero commits that's correctly dormant is healthier than one with
50 commits and no finish lines crossed.

**Prescribing fixes.** This step diagnoses. It doesn't prescribe. "This
wave needs design thinking" is a diagnosis. "You should split item 3
into two items and rewrite the README" is a prescription. Stay on the
diagnosis side.

**False precision.** Three levels (quiet/warm/hot) is enough. Don't
invent a 1-10 scale or weighted scoring formula. The value is in the
evidence and interpretation, not the number.
