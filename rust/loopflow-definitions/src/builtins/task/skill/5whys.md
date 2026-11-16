---
requires: a failure and what is known about it
produces: the causal chain and proposed prevention
---
Investigate why a failure happened and what would prevent it recurring.

Start from the report and what is already known, whether the failure is fixed or
still open. Say which it is; analysis alone fixes nothing. Read `scratch/` for
the current design and earlier notes.

## Philosophy

**Follow causes past the first patch.** Ask what made the failure possible and
whether changing that would prevent it. Depth comes from what you find, not a
required number of whys.

**Investigate before you document.** Read the code. Run the commands. Trace the
actual execution. The chain should reflect what you found, not what you guessed.

**"Human error" is never the root cause.** If someone made a mistake, ask why it
was possible. What guardrail was missing? What feedback loop failed?

**Look for leverage at every level.** How could prompts, processes, code or tests
take a better course? The best fixes aren't patches; they make the failure mode
impossible.

## Workflow

1. Understand the symptom: read errors, logs, code.
2. Ask "why did this happen?" and trace the answer. Don't guess.
3. Ask "why?" of that answer.
4. Follow the chain as far as what you find supports it.
5. Look back: what questions did you skip? What branches are unexplored?
6. At each level, ask what change to prompts, process, migrations, code or tests
   would have prevented this.

## Output

Update the existing incident note, or write a topic-named note under `scratch/`.
Record what can't be found again later: the original observations, what each
run showed at the time, and what people said, word for word. Don't
restate code or history a later agent can read. Include:

- What failed, who it affected, and whether it is fixed now.
- The causal chain, with what backs each link. Follow useful branches; don't
  make up a fifth cause or force every failure into a systemic one.
- Open questions, and anything that contradicts the leading explanation.
- Prevention sized to the failure, why it addresses the cause, and how to check
  it worked. Keep proposals apart from fixes already made, and quote what
  people said about them rather than calling them decided.

Stop where the facts stop. If no further change is worth making, say why. End
with the note's path and the next useful action, so planning can use the
findings without treating every proposal as approved work.
