# Prompt Style Guide

How to write prompts for loopflow. Prompts are instructions for LLM sessions—human-readable, but optimized for agents to follow.

## Structure

Every prompt starts with YAML frontmatter:

```yaml
---
requires: diff vs main | existing code | none
produces: scratch/something.md | code changes | verdict
interactive: true  # optional, default false
---
```

Then a one-line summary immediately after the closing `---`. This line captures the essence—what the prompt is for. No preamble, no "This prompt will...". Just state it.

```markdown
---
requires: diff vs main
produces: simplified code
---
Simplify code touched by this branch while preserving user behavior.
```

## Sections

Common sections, in typical order:

- **Goal**: Why this prompt exists. What success looks like.
- **Workflow**: Numbered steps. Concrete commands where relevant.
- **What matters / What to X**: Priorities. What to focus on.
- **What doesn't matter / Guardrails**: Constraints. What to avoid.
- **Output**: What artifact(s) to produce. Format if relevant.

Not every prompt needs every section. Short prompts can skip Goal if the opening line is clear enough.

## Voice

**Direct and imperative.** "Run tests." "Read the diff." Not "You should run tests" or "It's a good idea to read the diff."

**No identity framing.** Don't tell the agent what it is. "You are a code reviewer..." or "Your role is to..." assigns identity. Just instruct. Using "you" is fine—"Run tests and verify you see green" is direct and clear.

**Write for humans and agents alike.** The same words should make sense whether read by a person or executed by an LLM. "Identify where architecture and product intent are misaligned" works for both. "Think about what the user might want" doesn't.

**Opinionated, not balanced.** If there's a right way, say so. "The best reduction isn't deleting a function—it's reshaping a structure so three special cases become one."

**Concise but not terse.** Say what needs saying. Don't pad, but don't strip useful context either.

**Active voice.** "Write the review" not "The review should be written."

## Gate vs Big prompts

Two modes for the same concern:

**Gate prompts (`-gate`)**: Fast quality checks for inner loops.
- Decisive: produce a clear verdict (SHIP/ITERATE, DONE/MORE, etc.)
- Scoped: only look at what this branch changed
- Minimal output: verdict + issues if any
- "Can we ship this?"

**Big prompts (`-big`)**: Strategic assessment for steering.
- Broad: look at the whole codebase or system
- Produce documentation for human review
- Identify highest-leverage changes
- "What should we focus on?"

## Outputs for humans

Prompts are executed by agents, but outputs are read by humans. Keep both audiences in mind:

- CLI commands should be copy-pasteable
- Output formats should be scannable
- Verdicts should be unambiguous
- Design docs should stand alone

When in doubt about output format, optimize for the person who'll read it.

## Examples

Opening lines that work:
- "Does this work? Ship or iterate?"
- "Simplify code touched by this branch while preserving user behavior."
- "Look at this codebase through a user's eyes—human or digital."
- "Investigate the approach in the current diff and consider alternatives."

Opening lines that don't work:
- "You are a code reviewer who..." (don't "You are")
- "This prompt helps with..." (don't explain the prompt)
- "Please review the code and..." (no "please", just instruct)

## Checklist

Before committing a prompt:

- [ ] Frontmatter has `requires:` and `produces:`
- [ ] Opening line states the purpose directly
- [ ] No identity framing ("You are...", "Your role is...")
- [ ] Language works for both human readers and LLM executors
- [ ] Workflow steps are concrete and numbered
- [ ] Output format is specified if the prompt produces artifacts
- [ ] Gate prompts have clear verdicts
- [ ] Big prompts produce documentation, not just observations
