---
interactive: true
requires: diff vs main
produces: code changes | design doc | nothing
---
Walk the human through the current diff. Evaluate whether the model is right, the design is clean, and the change is ready.

If `scratch/<branch>-review.md` exists (from gate), use it as the briefing. Otherwise, read the diff cold.

## Arc

Each phase is a conversation pause. Present findings, wait for reaction, adjust. Don't monologue through all six — pause after each and let the human steer.

### 1. Orient

Summarize the shape of the change in 2-3 sentences. What's new, what moved, what's the intent. If the gate briefing exists, draw from it. Don't recite the diff.

### 2. Demo plan

Propose a concrete test plan the human can execute. Three parts:

**Setup.** Spell out every process that needs to be running and every prep step before the first test command. Don't assume the server is already up. Include build commands, daemon starts, seed data, environment variables — everything from cold start to ready.

**Test commands.** Specific commands to run, workflows to walk through, UX to exercise. Each command should include what to expect — status codes, output shapes, state changes. If there are multiple user paths, list them.

**Validation.** How to inspect logs, database state, or output to confirm the change works correctly. Specify log locations, what patterns to grep for, and what "healthy" looks like versus failure modes.

If the test workflow is more than a few commands, propose a Python script in `scripts/` that handles setup, exercise, validate, and teardown. The bar: an LLM agent should be able to run it, read the output, and know whether things are working without asking for help. E2e tests in `tests/e2e/` are trivial shell one-liners (`uv run python scripts/...`) that exist only as CI entry points — the scripts do the real work.

### 3. Core model

Walk through the central data structures and APIs at the heart of the change. Explain the model, then ask: is this the clearest possible expression of the product semantics? Are names right? Are boundaries between types right? Does the type hierarchy match how users think about this? If this wave has a Vision, does the model serve it?

### 4. Simplify

Propose concrete alternatives that could shrink or clarify the model. Show what a simpler version would look like — different type hierarchies, merged structs, eliminated indirection, fewer API surfaces. Not "you could simplify this" — sketch the code.

### 5. Contentious calls

Surface decisions that reasonable people would disagree on. Naming, scope boundaries, error handling strategies, public API shape, performance tradeoffs. Frame each as "here's the tradeoff" not "here's what's wrong." Check against the wave's Goals and Risks — do any decisions conflict?

### 6. Learnings

What did building this reveal? About the codebase, the product model, the approach. What would we do differently next time? What assumptions were validated or invalidated? Should the wave's Risks, Goals, or Metrics be updated based on what we learned?

## Guidance

- Focus on structural decisions, not formatting or style. Gate already handled polish.
- If something should change, change it directly or propose a design doc. No review artifacts.
- The gate doc is the agenda, not a script. Skip sections that don't apply.
- Read every changed file, but focus attention on new types, new public APIs, and changed signatures. Mechanical changes (imports, formatting) aren't worth discussing.
- When proposing simplifications, be concrete. Show the alternative type or signature, not just "this could be simpler."
- Quote the diff when discussing specific decisions. Make it easy to see what you're referring to.
