---
requires: scratch/tend-chord.md (with verdicts)
produces: wave/ (updated configs and items), scratch/tend-applied.md
---
Execute approved mutations from the chord.

## Goal

The human reviewed the chord and marked each mutation as apply, defer, or
reject. This step executes the approved mutations — editing wave configs,
updating items, restructuring directories. Mechanical execution, no
reinterpretation.

## Workflow

1. **Read the annotated chord.** `scratch/tend-chord.md` should have verdicts
   on each mutation. If a mutation has no verdict, treat it as deferred.

2. **Execute approved mutations.** For each mutation marked **apply**:

   Apply the lever:

   | Lever | Action |
   |-------|--------|
   | **Direction** | Edit `wave/<name>/<name>.yaml` — update `direction:` field |
   | **Area** | Edit `wave/<name>/<name>.yaml` — update `area:` field |
   | **Flow** | Edit `wave/<name>/<name>.yaml` — update `flow:` field |
   | **Items** | Create, edit, delete, or reorder files in `wave/<name>/` |
   | **Agent** | Edit `wave/<name>/<name>.yaml` — update `agent:` field |
   | **Step agents** | Edit `wave/<name>/<name>.yaml` — update `step_agents:` |
   | **Triggers** | Edit `wave/<name>/<name>.yaml` — update `triggers:` |
   | **Lifecycle** | Create or delete wave directories; update chord-wave area |

3. **Verify each mutation.** After applying, read the changed file back and
   confirm it parses correctly. A broken YAML config is worse than no change.

4. **Update lfd state.** For mutations that change wave config fields
   (direction, area, flow, agent), call `lf ops update-wave` or the
   equivalent API to sync lfd's runtime state with the YAML on disk.

5. **Record what was applied.** Write a summary of executed changes.

## Output

Write `scratch/tend-applied.md`:

```markdown
# Chord Applied — <date>

## Applied
### <mutation title>
**Wave**: <name>
**Change**: <what was changed>
**Files modified**: <list>

## Deferred
<mutations deferred, with reason from review>

## Rejected
<mutations rejected, with reason from review>
```

Commit the wave config changes with message: `tend: apply chord`.

## What to avoid

**Reinterpretation.** Execute what was approved. If a mutation says "change
direction from `[ux]` to `[ux, clarity]`", don't decide `[clarity, ux]`
is better. The human already reviewed this.

**Partial application.** If a mutation can't be applied cleanly (file doesn't
exist, config has changed since the scan), skip it and note why in the
output. Don't apply half a mutation.

**Silent failures.** Every mutation gets a line in the output — applied,
skipped, or failed. The human should be able to diff the chord against
the applied log and see exactly what happened.
