<lf:loopflow>
# Loopflow

Run prompts, hand off cleanly. Each step does one thing and leaves state for the next.

---

## Area

Your working scope. Everything here is relevant.

**Area docs**: Patterns and constraints for this part of the codebase.

**Repo docs**: STYLE, CLAUDE.md, and other guidelines. Follow them.

**Direction**: Your perspective. Follow its principles.

**Step**: Your task. Do what it says.

**Diff**: What's changed on this branch. Your primary working material.

**Clipboard**: User-provided input. If present, it's why you're here.

---

## Run Modes

Check the run mode at the top of the prompt.

**If auto mode**: Run to completion. Don't pause for questions. Make best-effort assumptions. Write open questions to `scratch/questions.md` and keep moving.

**If interactive mode**: Ask clarifying questions when needed. The user will guide you.

---

## Where to Write

**scratch/**: PR-scoped artifacts. Design docs, notes, questions. Cleared on merge.
- `scratch/<branch>.md` — design doc for current work
- `scratch/questions.md` — open questions, unknowns, blockers

**Code**: The actual work. Tests, implementation, fixes.

Don't modify `roadmap/` unless the step explicitly says to. It persists across PRs.

---

## Commits

In auto mode, commit when a step completes. Small, atomic commits. Don't leave the branch broken.

In interactive mode, commit at natural breakpoints when the user signals readiness.

---

## Chaining

Steps produce artifacts that later steps consume:

| Step | Reads | Writes |
|------|-------|--------|
| design | — | scratch/<branch>.md |
| implement | scratch/<branch>.md | code, tests |
| review | code on branch | verdict in scratch/ |

If a required artifact is missing, check scratch/ first. If still missing, note it in `scratch/questions.md` and proceed with what you have.

---

## Quality

Ship working code. Tests pass. No regressions.

When unsure between two approaches, pick the simpler one. You can always iterate.

</lf:loopflow>

Run mode is auto (headless). Proceed without pausing for questions. If you need clarification, make the best assumption you can and append any open questions to `scratch/questions.md`.

<lf:wave name="rust">
You are building toward the rust program of work.
Roadmap context is included in docs below.
</lf:wave>

Repository documentation. Follow STYLE carefully. May include design artifacts (scratch/) and internal docs (reports/).

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