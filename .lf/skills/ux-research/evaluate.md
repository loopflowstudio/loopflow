---
requires: scratch/ux-research/loop-NN/candidates.md
produces: scratch/ux-research/loop-NN/evaluations.md
---
Simulate **each persona** using **each candidate** to perform this loop's target
behavior. Record concrete likes/dislikes and a verdict per (persona × candidate).

## How to simulate honestly

For each persona × candidate, put yourself in that persona's context — their
goals, their repo/wave scale, their tolerance for chrome (see
`scratch/ux-research/personas.md`) — and walk them through the exact behavior
from the proposal on that candidate's screen.

Rules that keep this from becoming theater:

- **Ground every reaction in a specific.** "The `waitingReason` chip tells Tess
  the wave is blocked on PR limit without opening it" — not "good information
  scent." If you can't point at a concrete element or step, cut the reaction.
- **Let personas disagree.** A design that Kai loves and Maya bounces off is a
  *finding*, not a problem to smooth over. Surface the disagreement.
- **Answer the proposal's questions** (discoverability / legibility / friction)
  through the persona's eyes, not in the abstract.
- **Be willing to dislike the shipped shape.** If the current
  repo-sidebar-first layout loses to an alternative for this behavior, say so.

## Output

`scratch/ux-research/loop-NN/evaluations.md`:

```markdown
# Loop NN — Evaluations

## <Persona> × Candidate <X>
**Likes:** <concrete, grounded>
**Dislikes:** <concrete, grounded>
**Verdict:** <one line — does the behavior succeed for this persona here?>

... (every persona × every candidate)

## Cross-cutting
### Where personas agreed
### Where personas split (the real tensions)
### Per-candidate standing
<which candidates carried which personas, and why>
```

The most valuable output is the **split**: name the tension that a human now
has to resolve. Don't resolve it here.
