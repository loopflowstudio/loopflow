---
requires: wave/<wave>/ items
produces: scratch/<slug>.md
default_agent: claude
fast-path: lf ops ingest
---
Pick the highest-priority item from the wave's backlog and move it to scratch/.

## Wave context

**Finding the wave name:**
1. Check for `<lf:wave name="...">` tag in the prompt — this is the authoritative source
2. If no tag, look at the branch name pattern: `<wave>.main` indicates wave `<wave>`
3. If still unclear, check `wave/` for subdirectories — each subdirectory is a wave

The wave's plan (`wave/<wave>/`) should be included in docs. If you can't find the wave's plan, note this in `scratch/questions.md`.

## Bucketed wave plans

Wave plans use four semantic priority buckets:

```
wave/rust/
  README.md               # Strategic context (not a pickable item)
  p0-fix-crash-loop.md   # Broken / unblock-now work
  p1-core-engine.md      # Clear next steps
  p2-hardening.md        # Big "when not if" bets
  p3-experiments.md      # Speculative ideas
```

**Bucket ordering rules:**
- Pick from the highest-priority non-empty bucket first (`p0` before `p1`, `p1` before `p2`, `p2` before `p3`)
- Treat the prefixes semantically, not as a fake exact queue
- README.md provides principles and success criteria—use it to evaluate priority, but don't pick it

**Using README.md:**
- Read **Vision** to understand what the wave is trying to achieve
- Read **Goals** to evaluate priority — what moves success criteria most?
- Read **Risks** to evaluate urgency — is something blocked or at risk?
- Read **Metrics** to understand what signals matter
- Respect scope boundaries stated in Vision — don't pick items that conflict

**Using the roadmap (`p0-*.md` through `p3-*.md`):**
- The roadmap is the bucketed files alongside the README — their prefixes define urgency, not a total order
- Read them to understand dependencies and what's been shipped

## Selection criteria

Within the highest-priority non-empty bucket, evaluate each item:

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the wave's success criteria forward?

**Readiness.** Are prerequisites met? Is scope clear enough to start?

**Fit.** Does it match the current area or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Workflow

1. Get wave name from `<lf:wave>` in context
2. Find `wave/<wave>/` in the docs
3. Read README.md for strategic context (Vision, Goals, Risks, Metrics)
   Read the roadmap (`p0-*.md` through `p3-*.md`) for urgency, dependencies, and scope
4. Identify the highest-priority non-empty bucket
5. Pick the highest-priority item from that bucket
6. Move it to `scratch/<wave>-<slug>.md`

## Output

The selected item is moved to `scratch/<wave>-<slug>.md`. The original is removed from `wave/<wave>/`.

**If the wave backlog is empty:** The wave has nothing to build. This is not an error.
- If the wave is a chord member, it enters silence — alive, watching its area,
  but not proposing work. Write nothing. The wave stays ready for items to be
  added by the chord's tend flow or by the human.
- If the wave is standalone, signal completion by writing nothing. When used in
  a `loop_until_empty` flow, this signals the loop should terminate.

**If items exist but none are ready:** Write `scratch/questions.md` explaining what's blocking progress.

**If items exist but none are compelling:** This is distinct from "not ready."
A wave exists in a market for the user's attention. Shipping mediocre work is
worse than silence — it trains the user to ignore the wave. If no item would
deliver a genuinely compelling user experience, write nothing. Prioritize
validating existing experiences over building new ones.
