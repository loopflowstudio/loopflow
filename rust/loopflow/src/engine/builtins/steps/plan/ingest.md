---
requires: wave/<wave>/ items
produces: scratch/<slug>.md
---
Pick the highest-priority item from the wave's backlog and move it to scratch/.

## Wave context

**Finding the wave name:**
1. Check for `<lf:wave name="...">` tag in the prompt — this is the authoritative source
2. If no tag, look at the branch name pattern: `<wave>.main` indicates wave `<wave>`
3. If still unclear, check `wave/` for subdirectories — each subdirectory is a wave

The wave's plan (`wave/<wave>/`) should be included in docs. If you can't find the wave's plan, note this in `scratch/questions.md`.

## Staged wave plans

Wave plans may use numbered prefixes to indicate stages:

```
wave/rust/
  README.md          # Strategic context (not a pickable item)
  01-protocol.md     # Stage 1 items
  02-core-engine.md  # Stage 2 items
```

**Stage ordering rules:**
- Pick from the lowest-numbered stage first (01-* before 02-*)
- Only move to the next stage when the current stage is complete
- README.md provides principles and success criteria—use it to evaluate priority, but don't pick it

**Using README.md:**
- Read **Vision** to understand what the wave is trying to achieve
- Read **Goals** to evaluate priority — what moves success criteria most?
- Read **Risks** to evaluate urgency — is something blocked or at risk?
- Read **Roadmap** to understand sequencing and dependencies
- Respect scope boundaries stated in Vision — don't pick items that conflict

## Selection criteria

Within a stage, evaluate each item:

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the wave's success criteria forward?

**Readiness.** Are prerequisites met? Is scope clear enough to start?

**Fit.** Does it match the current area or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Workflow

1. Get wave name from `<lf:wave>` in context
2. Find `wave/<wave>/` in the docs
3. Read README.md for strategic context (Vision, Goals, Risks, Roadmap)
4. Identify the current stage (lowest numbered prefix with items)
5. Pick the highest-priority item from that stage
6. Move it to `scratch/<wave>-<slug>.md`

## Output

The selected item is moved to `scratch/<wave>-<slug>.md`. The original is removed from `wave/<wave>/`.

**If the wave backlog is empty:** Signal completion by writing nothing. This is not an error—it means the wave's work is done. When used in a `loop_until_empty` flow, this signals the loop should terminate.

**If items exist but none are ready:** Write `scratch/questions.md` explaining what's blocking progress.
