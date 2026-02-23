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

Propose specific commands to run, workflows to walk through, UX to exercise. The human should know exactly how to poke at this change. If there are multiple user paths, list them.

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
