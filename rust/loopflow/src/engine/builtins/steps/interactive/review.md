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

Produce a runnable script, not a list of commands. The human should be able to run one command and immediately start the manual walkthrough.

**Default: write or extend a script in `scripts/`.** Check `scripts/` first — reuse or extend an existing script if one covers similar ground. The script handles automated checks (build, lint, test) and ends by launching whatever the human needs for manual verification (e.g., `concerto-dev.py run-debug` for UI work). The bar: run one command, get a working environment, start clicking.

The script should:
- Run automated checks (fmt, clippy, cargo test, swift test, python tests — whichever apply)
- Print a clear pass/fail summary
- If all pass, launch the manual environment (lfd + Concerto, or whatever the change needs)
- Print a short walkthrough checklist inline before launching

**Manual walkthrough checklist.** After the script launches the environment, tell the human what to exercise. Be specific about what to look for — UI states, expected behavior, edge cases. Keep it short enough to scan in 30 seconds.

**When a script isn't needed.** If the change is purely backend with no manual verification beyond tests passing, skip the script — just confirm the automated suite covers it.

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
