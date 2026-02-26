---
interactive: true
requires: diff vs main
produces: code changes | design doc | nothing
action_style: procedural
---
Walk the human through the current diff and help them decide the next right move.

If `scratch/<branch>-review.md` exists (from gate), use it as the briefing. Otherwise, read the diff cold.

## Approach

Use a natural structure that fits this diff. Don't force a fixed protocol or rigid output format.

Pause often. Present one chunk, get reaction, adapt. Keep momentum without turning this into a template exercise.

Pick the lenses that matter most for this change. Combine or skip lenses as needed:

- **Shape and intent** — summarize what's new, what moved, and the core intent.
- **Confidence and demo path** — show how to verify behavior quickly.
- **Model quality** — assess data structures, API boundaries, and naming clarity.
- **Simplification opportunities** — show concrete alternatives, not abstract advice.
- **Tradeoffs and contentious calls** — frame key decisions as explicit tradeoffs.
- **Execution path** — decide what to fix now vs defer to the wave roadmap.

## Collaborative execution loop

Use review to move the branch forward, not just discuss it.

During the session:
- Fix straightforward issues directly.
- Co-design unresolved decisions with the user when tradeoffs are non-obvious.
- Offer concrete package options and let the user choose:
  - **Minimal** — smallest safe ship-now set.
  - **One more big push** — one additional meaningful improvement pass, then ship.
  - **Do it all** — complete scope now and accept longer cycle time.
- Confirm the user has ingested the changes and validated the updated design/behavior.

Target outcome: the PR is ready to ship.

## Verification expectations

**Default: write or extend a Python script in `scripts/` (no bash).** Check `scripts/` first — reuse or extend an existing script if one covers similar ground. The script should launch whatever the human needs for manual verification (e.g., `concerto-dev.py run-debug` for UI work). The bar: run one command, get a working environment, start clicking.

The script should:
- Focus on manual/live review flows, not CI reproduction
- Avoid full automated test/lint suites unless the human explicitly asks
- Launch the manual environment (lfd + Concerto, or whatever the change needs)
- Print a short walkthrough checklist inline before launching

**Manual walkthrough checklist.** After the script launches the environment, tell the human what to exercise. Be specific about what to look for — UI states, expected behavior, edge cases. Keep it short enough to scan in 30 seconds.

**When a script isn't needed.** If the change is purely backend with no manual verification, skip the script and explain why.

## Quality coverage

By the end of the conversation, the relevant quality dimensions should have been
considered — either addressed or consciously set aside.

If directions are loaded, they define the quality lens. Otherwise, make sure these
areas got appropriate attention:

- User experience (visibility, feedback, consistency)
- Correctness and test confidence
- Reliability, performance, security
- Modularity and change impact

No mandatory format. If a dimension isn't relevant, that's fine — just be sure
it's a conscious choice, not an oversight.

## Guidance

- Focus on structural decisions, not formatting or style. Gate already handled polish.
- If something should change, change it directly or propose a design doc. No review artifacts.
- The gate doc is the agenda, not a script.
- Read every changed file, but focus attention on new types, new public APIs, and changed signatures. Mechanical changes (imports, formatting) aren't worth discussing.
- When proposing simplifications, be concrete. Show the alternative type or signature, not just "this could be simpler."
- Quote the diff when discussing specific decisions. Make it easy to see what you're referring to.
