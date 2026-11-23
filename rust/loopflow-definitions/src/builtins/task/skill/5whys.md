---
requires: a failure and what is known about it
produces: the causal chain and proposed prevention
---
Investigate why a failure happened and what would stop it happening again.

Start from the report and what is already known. Say whether the failure is
fixed or still open; analysis alone fixes nothing.

**Investigate before you write.** Read the code, run the commands, trace the
actual execution. The chain reflects what you found, not what you guessed.

**Follow causes past the first patch.** Ask what made the failure possible,
then ask why of that answer. Depth comes from what you find, not from a
required number of whys. Don't invent a fifth cause or force every failure
into a systemic one. Stop where the facts stop.

**"Human error" is never the root cause.** If someone made a mistake, ask why
it was possible. What guardrail was missing? What feedback never arrived?

**Look for leverage at every level.** At each link, ask what change to prompts,
process, code or tests would have prevented it. The best fixes aren't patches;
they make the failure impossible.

Before finishing, look back: which questions did you skip, which branches are
unexplored?

Update the existing incident note, or write one under `scratch/`:

- What failed, who it affected, and whether it is fixed now.
- The causal chain, with what backs each link, and anything that contradicts
  the leading explanation.
- Prevention sized to the failure, why it addresses the cause, and how to tell
  it worked. Keep proposals apart from fixes already made; a proposal is not
  approved work.

Record the original observations and what each run showed at the time; they
can't be found again. If no further change is worth making, say why. End with
the note's path and the next useful action.
