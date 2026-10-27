---
docs: scripts/testing.md
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
- Read the repo's agent doc (`AGENTS.md`) for conventions.

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

   Exercise changed interactions through automated UI tests or rendered snapshots.
   Leave subjective experience and manual walkthroughs to demo/review.

4. **Tests and lints**
   Run the affected suites and required formatting/static-analysis checks once
   for the current tree, including the design's automated acceptance. Plan any
   relevant before/after measurements and headless UI captures in this same run.
   - This repository's commands are in the included `scripts/testing.md`;
     use them over the general guidance below where they differ.
   - Plan from the diff before running anything: list the changed paths, the
     suites they can reach, and why. Run that list, not the whole repository.
   - Find the repository's changed-aware runner (a test script, task runner or
     CI path filter) in its agent guide, module READMEs or CI configuration.
     Prefer it over hand-picked commands; if it can reuse a passing result only
     for identical tracked/untracked content and the same plan, enable that reuse.
   - A later pass reruns only the suites whose inputs changed since the last
     pass. Unchanged suites keep their result.
   - Never run a whole package or workspace test suite to stand in for focused
     ones. Run the full matrix only when reproducing a CI failure or when release
     guidance requires it; CI owns it otherwise.
   - One build at a time: never run two compilers, test runners or linters
     against the same build directory concurrently, in the background or in
     parallel tool calls. Run tests first, then static analysis that reuses the
     same build.
   - Leave slow integration binaries the change cannot reach to CI.
   - Cross-check CI so formatting and static-analysis commands match what the
     repository enforces. Use auto-fix modes where available; fix the rest by hand.
   - All required checks must run headless without a person or display session.
     For Desktop, build the app and run view/interaction tests or offscreen
     snapshots. Do not require manual clicks or permission dialogs.
   - If this environment cannot run a check, use a headless equivalent or defer
     it to capable CI and continue the Graph. Never mark a deferred check passed;
     CI still owns its result. Leave display-dependent judgment to demo/review.
   - Record the command and result in one line; include any CI deferral.
   Fix failures—determine whether it's broken test or broken code. Add tests for key behavior changes. Keep them focused. Delete flaky tests rather than patching them.
   A timeout on a required check gets investigated, not overridden by a later
   green run: name what ran long and why before the branch ships.

5. **Cleanup**
   - Remove dead code, debug prints, resolved TODOs
   - Remove backwards-compatibility shims that aren't needed (old parameter names, deprecated re-exports, migration code for formats nothing uses)
   - Consistent formatting in changed files
   - No leftover comments like `// TODO: remove this`

## Phase 2: Polish Docs

Make the change easy to review.

1. **Assess the results**
   Compare the checks above with the accepted outcome. Record meaningful
   before/after numbers and any CI deferral; leave judgment to demo/review.
   Do not start a second verification pass. For substantial UI work, identify
   its production performance signal; propose one for the Wave when associated.

2. **Prepare delivery context**

   Keep the experienced improvement, actual checks and material limits in the
   existing plan or durable docs. Delivery commands generate PR copy through
   their pr-message template; gate need not duplicate that writing pass.
   If copy was explicitly requested or already prepared, reconcile it with the
   current change. Optional cached copy lives in `scratch/pr-title.txt`,
   `scratch/pr-body.md` and `scratch/.pr-copy-ref` (current HEAD SHA).
   Publication consumes these files before its first commit or push. Preserve
   consequential evidence at its lasting owner before cleanup of added scratch files.

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
leave them alone. Report check results and unresolved blockers briefly.

## Adaptation

Did you discover a quality check this repo always needs? A formatter, a type
check, a build step that should run every time? Encode it so the next gate is
faster. Commands belong in `scripts/testing.md`, which gate and implement
share; boundary checks belong in the owning module README. This is Loopflow's
copy of the builtin gate; keep the two in step.
