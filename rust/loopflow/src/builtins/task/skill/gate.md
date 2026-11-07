---
requires: code on branch
produces: corrected code and docs, check results, and context for the PR
action_style: procedural
---
Make the branch ready to ship and easy to review.

Read `scratch/` (`scratch/<branch>.md` is this PR's design, `scratch/questions.md`
holds open questions) and the repo's agent guide. Read Wave or Linear context
only when the seed names the exact wave, Task or Project; never infer it.

## Phase 1: Polish code

1. **Review the diff**, including uncommitted work, against the intended outcome
   and the repo's style guides.

2. **Fix developer experience**: sensible defaults, obvious signatures, the same
   word for the same concept, structure that matches the mental model. If three
   functions take `(path, config, options)` and one takes `(config, path, opts)`,
   fix it.

3. **Fix user experience**: fast paths stay fast, errors are clear, nothing
   fails silently. Slow is a bug. Exercise changed interactions through
   automated UI tests or rendered snapshots; leave manual walkthroughs to demo.

4. **Tests and lints.** Run the affected suites, formatting and static analysis
   once for the current tree, including the design's automated acceptance and
   any before/after measurements.
   - Plan from the diff first: changed paths, the suites they reach, and why.
     Run that list, not the whole repository.
   - Prefer the repo's changed-aware runner (test script, task runner, CI path
     filter) over hand-picked commands, with result reuse for identical content
     and plan when it offers it.
   - A later pass reruns only suites whose inputs changed.
   - Never run a whole package or workspace suite in place of focused ones. The
     full matrix belongs to CI, except to reproduce a CI failure or when release
     guidance requires it.
   - One build at a time: never run two compilers, test runners or linters
     against the same build directory at once. Tests first, then static analysis
     on the same build.
   - Leave slow integration binaries the change cannot reach to CI.
   - Match formatting and lint commands to what CI enforces. Use auto-fix modes;
     fix the rest by hand.
   - Every check runs headless. For Desktop, build the app and run view and
     interaction tests or offscreen snapshots; no manual clicks or permission
     dialogs. If a check can't run here, use a headless equivalent or leave it
     to CI and keep going. Never mark a deferred check passed.
   - Record each command and result in one line, with any CI deferral.

   Fix failures: decide whether the test or the code is broken. Add focused
   tests for key behavior changes. Delete flaky tests rather than patching them.
   A timeout on a required check gets investigated, not overridden by a later
   green run: name what ran long and why before the branch ships.

5. **Clean up**: dead code, debug prints, resolved TODOs, backwards-compatibility
   shims nothing needs, comments like `// TODO: remove this`.

## Phase 2: Polish docs

1. **Assess the results** against the accepted outcome. Note meaningful
   before/after numbers and any CI deferral. Don't start a second verification
   pass. For substantial UI work, name its production performance signal, and
   propose one for the Wave when there is one.

2. **PR context.** Keep the improvement people will notice, the checks that ran
   and real limits in the plan or lasting docs. Delivery commands write PR copy
   from their pr-message template, so gate doesn't. If copy was requested or
   already exists, bring it up to date: `scratch/pr-title.txt`,
   `scratch/pr-body.md` and `scratch/.pr-copy-ref` (current HEAD SHA), which
   publishing reads before its first commit or push. Move anything worth
   keeping out of added scratch files before they are cleared.

3. **README and docs.** If user-facing behavior changed, the docs say so.
   Examples work and commands are current: `README.md`, module READMEs,
   docstrings on public APIs.

4. **Inline comments** where the why isn't obvious. `# increment counter` above
   `counter += 1` is noise.

## Scope

Polish only code this branch changed, within the design. Skip unrelated
improvements and style preferences. If it already meets the bar, leave it
alone. Report check results and blockers briefly.

## Adaptation

Found a check this repo always needs (a formatter, a type check, a build step)?
Write it into the repo's agent guide or module docs so the next gate is faster.
Copy this skill to `.lf/skills/gate.md` when the repo needs gate to work
differently.
