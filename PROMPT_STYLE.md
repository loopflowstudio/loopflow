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

**Dynamic, not formulaic.** Interactive prompts run repeatedly across different contexts. Each session should feel different — vary structure, entry point, and emphasis based on what's actually interesting here. Agents that follow the same script every time produce rubber-stamp outputs users stop reading.

## Goals

Goals shape judgment and intent—how an agent approaches work. They're different from steps (which define tasks) and flows (which chain steps).

### Structure

Goals follow a consistent pattern:

1. **Opening line**: What this goal is for. Direct, imperative.
2. **Success**: What "done" looks like. Specific enough to know when you've achieved it.
3. **The substance**: Principles, questions, or perspective. NOT numbered process steps.
4. **Quality bar**: Standards for the output.
5. **Anti-patterns**: Common failure modes to avoid. All goals should have these.

### Wave/project/task ontology

Wave planning uses three nouns, and they differ by kind rather than size:

- **Wave**: durable operating context. It owns memory, cadence, budget, chat,
  and project selection.
- **Project**: measured bet inside exactly one wave. It owns KRs and closure
  criteria.
- **Task**: concrete implementation, investigation, docs, or shipped change.

Every project belongs to one wave. Projects do not contain projects, and they do
not own their own memory or cadence. If a project seems to need subprojects,
either split it into sibling projects, promote the operating context into a
wave, or demote the pieces into tasks.

Good projects are either completable behavioral improvements or standing quality
frontiers. "Wave Chat can steer and interrupt work from CLI and Mac" is a
project. "Technical Architecture stays legible and minimally simple" is a
project. "Delete an obsolete API" is individual debt: file it as a task under
a project.

Write project KRs as proof, not backlog. A KR states an observable end state:
what would let a maintainer say the bet now holds. Do not mix task lists,
implementation receipts, issue ids, or status into the KR line.

```markdown
# Weak: task bundle pretending to be a project
# One system

## KRs

- Delete the obsolete HTTP read surface.
- Retire chord/member vocabulary.
- Unify the operating prompt.

# Strong: project frontier with proof-shaped KRs
# Technical Architecture

Loopflow's architecture is legible from the top down: the key data structures
and APIs explain the system, the implementation follows that map, and obsolete
pre-loop concepts do not linger as alternate design.

## KRs

- Top-down architecture documentation is complete, published, and centered on the key data structures and public APIs.
- Every data structure and API in the architecture is ratified as minimally simple for its purpose.
- The codebase, prompts, docs, and UI contain no stale pre-loop technical design language.
```

### A wave's GOAL.md: five marks of a goal that loops well

A wave's `GOAL.md` is a goal run in a loop. It defines the wave's operating
context: why it exists, what it owns, what it refuses, and how it chooses useful
work. Project KRs live with the project, not as a roadmap table in `GOAL.md`.
The difference between a wave that compounds and one that produces rubber-stamp
output comes down to five marks.

1. **Identity by contrast.** State what the wave is *and what it is not*, ideally
   against a sibling. "Systems owns the machinery around the code; Architecture
   owns the shape of the code." "Loopflow frames the vendor's session; it
   does not render chat." A named boundary is what stops a looping agent from
   sprawling into every adjacent wave's work.

2. **Measures are readable signals, most-important-first.** Not "the feature
   works" — numbers or checkable states the loop can actually read: `billing`,
   `prod uptime`, `green on main`, `test time`; `net concept count flat-or-falling`.
   Three to five, ranked. If a measure can't be observed, it can't steer a loop.

3. **The body is a loop with concrete verbs.** Give the agent a menu of real
   moves to choose among: "sand a sharp edge, automate a manual ritual, harden a
   flaky pipeline, or turn a failure into a fix PR." "Pick the next useful move"
   beats "make it better" — the second is unactionable at 2 a.m.

4. **An honest-question north that resists gaming.** Name the one check that a
   lazy loop can't fake. architecture's is sharp: "the honest question is never *how
   much did you delete* — it is *did a design ship and is the tree lighter a
   quarter later*." Without it, the loop optimizes the easy proxy.

5. **A stop discipline.** Tell the loop when *not* to invent work: "if no safe
   move remains, record the blocker instead of inventing work"; "done for now when
   the next move costs more than the entropy it removes." A goal with no off-ramp
   manufacts busywork forever.

```markdown
# Thin — technically valid, but it can't steer a loop
Read the live tasks, pick the next useful move, dispatch the appropriate flow, and
leave loopflow closer to done.

# Makeover — identity, verbs, and a north the loop can be held to
You keep the engineering outfit efficient — the machinery around the code, not
its shape (that's Architecture's job). Pick the next move: sand a sharp edge,
automate a manual ritual, harden a flaky pipeline, or turn a failure into a
focused fix PR. Keep the machinery boring and self-healing. If no safe move
remains, record the blocker instead of inventing work.
```

### Two kinds of goals

**Action goals** (adapt, wave-plan, ship): What mode to operate in. These can reference loopflow-specific process—reading `wave/`, updating frontmatter status, choosing between modes. They're about *using the system*.

**Perspective goals** (ux, infra, craft, ceo): How to think. These should be broad and transferable—they'd make sense at any company. Focus on judgment, values, and trade-offs. Minimal process.

### Principles over process

Goals primarily provide judgment, not workflow. Detailed process belongs in flows and steps.

"Where's the biggest gap between vision and implementation?" is a goal. "1. Read the wave plan. 2. Pick an item. 3. Update status to in-progress. 4. Build it. 5. Update status to done." is process—that belongs in a flow.

Action goals can include light process for system navigation. Perspective goals should be almost purely principles.

### Abstraction level

**Loopflow system concepts are fine:** `wave/`, `scratch/`, frontmatter, areas, flows—these are part of how loopflow works and belong in goals.

**Product-specific details should be abstracted:** Don't write "use `.monospacedDigit()` for numbers"—write "ensure numeric data is readable." Don't write "44pt tap targets"—write "touch targets big enough for humans on mobile, LLMs using browser tools, or any other user type."

The goal should work for any codebase using loopflow, not just the one where it was written.

### Orthogonality

Directions are orthogonal to steps and areas. A direction applies to any task in any scope.

```
Step = what you're doing (implement, review, design)
Area = where you're working (src/api/, swift/Loopflow/)
Direction = which users you're trying to serve
```

**Don't couple to steps.** "When reviewing code, ask..." ties the direction to `review`. The same concerns apply whether you're reviewing, implementing, or designing.

**Don't couple to areas.** "When working on Loopflow..." ties the direction to a specific codebase. User patterns like conductor/improviser/listener exist in any product with parallel work.

### How directions apply

Directions are not roleplay. A direction is intent—what you're optimizing for while doing your work.

Directions can be:
- **User patterns**: conductor, improviser, listener—make this kind of user thrive
- **Perspectives**: ux, infra, craft, ceo—think with these concerns
- **Metrics**: performance, security, accessibility—optimize for this quality
- **Values**: simplicity, craft—hold this standard

The questions in a direction help you check if your work serves the intent.

```bash
lf implement --direction conductor --area src/api/
```

You're implementing in src/api/. The conductor direction means you're building with the intent that conductors thrive. The questions ("can I see what needs attention without drilling in?") verify your implementation serves that intent.

```bash
lf review --direction security --area src/auth/
```

You're reviewing src/auth/. The security direction means you're optimizing for security. The questions surface vulnerabilities you might otherwise miss.

```markdown
# Bad: coupled to step and area
When reviewing Loopflow code, ask:
- Can I tell what needs attention?

# Good: intent + questions
Managing multiple parallel workstreams. Checking in, not diving deep.

- Can I see what needs attention without drilling in?
- Is urgency visually obvious?
- How many clicks from "I see a problem" to "I'm acting on it"?
```

### Voice

Goals should feel opinionated. "Slow is fake." "Errors of omission kill." "Sunk cost is sunk."

Use direct statements and sharp questions. Avoid hedging ("consider whether...", "you might want to...").

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

When a prompt produces a test plan or demo procedure, produce a runnable script in `scripts/` — not a list of commands. Check `scripts/` first and extend existing scripts when possible. The bar: one command to run, one environment to verify in.

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
