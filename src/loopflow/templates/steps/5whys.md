---
interactive: true
produces: scratch/5whys-<topic>.md
---
Perform root cause analysis on a bug, incident, or unexpected behavior using the 5 Whys technique. Produces a design doc that requires implementation.

## Philosophy

**Keep asking why until you feel resistance.** That resistance—"we'd have to change how we think about this"—is where the real fix lives.

**Investigate before you document.** Read the code. Run the commands. Trace the actual execution. The chain should reflect what you discovered, not what you guessed.

**"Human error" is never the root cause.** If someone made a mistake, ask: why was that mistake possible? What guardrail was missing? What feedback loop failed?

**Look for leverage at every level.** As you dig, ask: how could we change our prompts, processes, code, or tests to take a better course? The best fixes aren't patches—they're course corrections that make the failure mode impossible.

## Workflow

1. Understand the symptom thoroughly—read errors, logs, code
2. Ask "why did this happen?" and trace the answer (don't guess)
3. Take that answer and ask "why?" again
4. Keep asking until you hit something systemic
5. Look back: what questions did you skip? What branches unexplored?
6. For each level, ask: what change to prompts, process, code, or tests would have prevented this?

## Output format

Write to `scratch/5whys-<topic>.md`:

```markdown
# 5 Whys: <Problem>

## The Problem
<One sentence: what went wrong>

## Chain

Problem → Cause 1 → Cause 2 → Cause 3 → Root Cause

**Problem**: <observable symptom>

**Why 1**: <answer>
↳ *Could we have caught this earlier?*

**Why 2**: <answer>
↳ *What process allowed this?*

**Why 3**: <answer>
↳ *What assumption was wrong?*

**Why 4**: <answer>
↳ *Why was that assumption encoded?*

**Why 5 (Root)**: <systemic cause>

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 2 | Why didn't tests catch this? | High |
| Why 3 | Why did we choose X over Y? | Low |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | <quick unblock> | This specific instance |
| Structural | <code/process change> | This class of bugs |
| Systemic | <prompt/tooling/architecture> | Future similar issues |

## Changes to Implement

- [ ] <specific change 1>
- [ ] <specific change 2>
```

Run `lf implement` to execute the fixes.

## Conversation guidance

When the user says "I don't know why," investigate together. Read the file. Check git history. Run the test.

If you hit a cause outside your control (external dependency, hardware), pivot: "Given we can't change X, why were we vulnerable to it?"

At each level, explicitly ask: "What change to our prompts, processes, code, or tests would make this path impossible?"
