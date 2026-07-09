---
requires: wave/<chord>/, member wave configs, README goals, roster, direction config, algedonic history, recent chord mutations
produces: scratch/vsm-s5-scan.md
---
Scan chord identity state.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its `GOAL.md`, `MEMORY.md`, `projects/`, and live tasks (`lf op pm show --wave <name>`).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Observe the chord's identity and boundary without judging it yet.
Read what the chord says it is, which waves it currently owns, and what recent
structural pain or mutation patterns say about that identity.

## Workflow

1. Read the chord README and config.
2. Read each member wave's README, YAML, and current roster membership.
3. Read direction configs and any autonomy / trigger settings that shape policy.
4. Read algedonic history and recent repair / escalation patterns.
5. Read recent chord mutations since the last s5 cycle.
6. Record observations only — no proposals, no fixes.

## Output

Write `scratch/vsm-s5-scan.md`:

```markdown
# VSM S5 Scan — <date>

## Chord Identity
<vision, strategy, stated goals>

## Boundary and Roster
<which waves exist, what territory they own, where overlap or gaps appear>

## Policy Signals
<direction, trigger, autonomy, escalation patterns>

## Recent Structural Changes
<recent mutations or notable shifts since the last cycle>

## Raw Signals
<facts and anomalies only>
```

## What to avoid

**Assessment language.** This step scans. It does not decide.
