---
requires: code on branch
produces: corrected code and docs, verification evidence, and delivery context
action_style: procedural
---
Make the branch as ready to ship as possible, and as easy for reviewers to evaluate as possible.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Phase 1: Polish Code

1. **Review the diff**
   Inspect the supplied change, including uncommitted work, against its intended
   outcome and the repo's style guides. Resolve the relevant comparison from the
   working context; do not assume a particular base branch or prior skill.

2. **Fix developer experience**
   - Intuitive APIs: sensible defaults, obvious signatures, no surprises
   - Consistent naming: same concept, same word, everywhere
   - Clean structure: code organization matches mental model

   Example: If three functions take `(path, config, options)` and one takes `(config, path, opts)`, fix it.

3. **Fix user experience**
   - Fast paths stay fast. If a flow added latency, find it and fix it.
   - Errors are clear. No silent failures, no cryptic messages.
   - Interactions feel snappy. Slow is a bug.

   Example: Run through the main user flows the branch touches. Click every button. Time the response. If something feels sluggish, profile it.

4. **Tests and lints**
   Run the affected suites and required formatting/static-analysis checks once
   for the current tree.
   - Follow the repo's documented guidance first (`TESTING.md`, `README.md`, and relevant module docs).
   - Cross-check CI so formatting and static-analysis commands match what the
     repository enforces.
   - Use the repo's standard command entrypoints and auto-fix modes where
     available. Fix remaining formatting or static-analysis failures manually.
   - Prefer a changed-aware runner. If it can reuse a passing result only for
     identical tracked/untracked content and the same plan, enable that reuse.
   - Do not run the full local matrix merely to mirror parallel CI. Run it only
     when release guidance requires it or when reproducing a full-matrix failure.
   - Record what was selected, reused, or deliberately left to CI.
   Fix failures—determine whether it's broken test or broken code. Add tests for key behavior changes. Keep them focused. Delete flaky tests rather than patching them.

5. **Cleanup**
   - Remove dead code, debug prints, resolved TODOs
   - Remove backwards-compatibility shims that aren't needed (old parameter names, deprecated re-exports, migration code for formats nothing uses)
   - Consistent formatting in changed files
   - No leftover comments like `// TODO: remove this`

## Phase 2: Polish Docs

Make the change easy to review.

1. **Verify the intended outcome**
   - Check the accepted success conditions. Reuse applicable passing evidence;
     run missing proof and record its limits in the PR copy or existing plan.
   - If the work has measurable outcomes (performance, accuracy, latency, size, counts), run before/after comparisons and record the numbers
   - If the work is a UI or UX change, capture the key states and interactions
   - Not every PR has metrics — but when they exist, capture them now. The reviewer shouldn't have to reproduce your setup to see the impact.
   - For substantial UI work, state which production performance signal covers
     the new path. If none does, capture a concrete metric proposal for the
     Wave. A local benchmark supports it but is not live coverage by itself.

2. **Prepare delivery context**

   Keep the experienced improvement, actual checks and material limits in the
   existing plan or durable docs. Delivery commands generate PR copy through
   their pr-message template; gate need not duplicate that writing pass.
   If copy was explicitly requested or already prepared, reconcile it with the
   current change. Optional cached copy lives in `scratch/pr-title.txt`,
   `scratch/pr-body.md` and `scratch/.pr-copy-ref` (current HEAD SHA).
   Publication consumes these files before its first commit or push. Preserve
   consequential evidence at its lasting owner before delivery clears scratch.

3. **Update README and docs**
   - If user-facing behavior changed, docs must reflect it
   - Examples must work. Commands must be current.
   - Check: `README.md`, module READMEs, docstrings on public APIs

4. **Inline documentation**
   - Add comments where the "why" isn't obvious
   - Don't document the obvious. `# increment counter` above `counter += 1` is noise.

## Scope

Polish only code changed by this branch, within the design intent. Skip unrelated
improvements and style preferences. If code and docs already meet the contract,
leave them alone. Report applicable proof and unresolved blockers briefly.

## Adaptation

Did you discover a quality check this repo always needs? A formatter, a type
check, a build step that should run every time? Encode it so the next gate is
faster. Most discoveries belong in repo docs (CLAUDE.md, TESTING.md) where all
skills can see them. Copy this skill to `.lf/skills/gate.md` when the repo needs
gate to work differently.
