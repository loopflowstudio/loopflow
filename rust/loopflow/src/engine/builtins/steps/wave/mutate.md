---
requires: scratch/garden-assessment.md or scratch/vsm-*-assessment.md
produces: wave/ (updated configs and items), scratch/wave-mutate.md
---
Compose and play the chord in one pass.

## Goal

The assessment already surfaced the pressure. Act on it now.

Compose the smallest coordinated set of mutations that addresses the current
pressure points or proposals, apply them immediately, and leave a clean record
of what changed and why. This step is both composer and performer.

## Workflow

1. **Read the freshest assessment.** Use the assessment produced by the current
   flow:
   - `scratch/garden-assessment.md`, or
   - `scratch/vsm-s5-assessment.md`, `scratch/vsm-s4-assessment.md`,
     `scratch/vsm-s3-assessment.md`, or `scratch/vsm-s2-assessment.md`

2. **Extract the actionable pressure.** Pull out only the items that warrant
   mutation now:
   - garden pressure points
   - s5 identity / boundary concerns
   - s4 environmental proposals
   - s3 mechanical or capacity fixes
   - s2 coordination fixes

3. **Compose the chord.** For each change you will make, specify:
   - target wave
   - lever (`direction`, `area`, `flow`, `items`, `agent`, `step_agents`,
     `triggers`, `lifecycle`)
   - before / after
   - rationale
   - risk

4. **Play it immediately.** Apply the mutations on disk:
   - edit wave YAML for config changes
   - create, update, reorder, or delete wave items
   - create or remove wave directories only when lifecycle pressure requires it

5. **Sync runtime state.** For changes that affect registered wave config,
   update runtime state through `lf ops update-wave` or the equivalent API.

6. **Verify.** Read back every changed config and make sure the YAML still
   parses. If a mutation cannot be applied cleanly, skip it and record why.

7. **Record the chord.** Write what you played, what changed, and what you left
   alone.

## Output

Write `scratch/wave-mutate.md`:

```markdown
# Chord Played — <date>

## Source
<which assessment drove this chord>

## Summary
<what pressure this chord addressed>

## Mutations
### 1. <title>
**Wave**: <name>
**Lever**: <direction | area | flow | items | agent | step_agents | triggers | lifecycle>
**Before**: <state before>
**After**: <state after>
**Rationale**: <why this change now>
**Risk**: <what could go wrong>
**Files changed**: <paths>
**Status**: applied | skipped
**Notes**: <verification or skip reason>

## Deferred
<findings that did not warrant mutation now>
```

Commit the applied changes with message: `wave: mutate`.

## What to avoid

**Two-step theater.** Don't draft a proposal and leave execution for later.
Play the chord now.

**Over-mutation.** Change only what the assessment actually justifies.

**Silent skips.** If a mutation was considered but not applied, say why.
