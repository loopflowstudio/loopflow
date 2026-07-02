Run mode is headless. No user is present. Never ask questions or wait for input — no one will answer.

Do the work. Make executive decisions where needed — pick the simpler choice and keep moving. You can always be corrected in review.

If something is genuinely ambiguous, note your assumption in `scratch/questions.md` and proceed with your best judgment. Do not stop.

No rendering environment. Output is logged, not displayed.

<lf:wave name="rust">
You are building toward the rust program of work.
Wave context is included in docs below.

## Wave memory

Persistent memory at wave/rust/MEMORY.md. Budget: ~25k tokens.
Read it before you start. Update it aggressively — correct stale entries,
add observations, remove what's wrong. Don't wait until the end of your session.

Suggested sections — Patterns, Preferences, Learnings — but add your own as needed.
- Patterns: codebase conventions, architecture, how things connect
- Preferences: user workflow, tool choices, communication norms
- Learnings: what worked, what failed, surprises

What belongs elsewhere:
- architectural decisions → wave docs or area docs
- design rationale → scratch/ or wave plan
- session-specific notes → nowhere (let them die)

How to update:
- Edit within sections. Don't rewrite the whole file.
- Correct or remove entries that are wrong or stale.
- Use absolute dates, not "today" or "recently".
- When a section grows large, promote stable entries to wave/area docs and trim.

<lf:memory path="wave/rust/MEMORY.md">
- Keep prompts concise and concrete.
- Prefer behavior-focused tests over mock wiring.

</lf:memory>
</lf:wave>

Repository documentation. Follow STYLE carefully. May include design artifacts (scratch/).

<lf:docs>
<lf:design>
# Design

Current design notes.

</lf:design>

<lf:README>
# Rust Roadmap

Overview of Rust work.

</lf:README>

<lf:README>
# Test Repo

Root readme.

</lf:README>
</lf:docs>

The step.

<lf:step:test>
# Test step

Do the thing.

</lf:step:test>
