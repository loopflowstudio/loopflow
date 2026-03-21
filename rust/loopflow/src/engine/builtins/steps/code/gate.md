---
requires: code on branch
produces: polished code, scratch/<branch>-review.md
default_agent: codex
action_style: procedural
---
Make the branch as ready to ship as possible, and as easy for reviewers to evaluate as possible.

## Goal

Polish isn't "tests pass." Tests passing is table stakes.

Polish means: the code is as good as it can be given the design intent, and a reviewer can understand the change in one read.

Ship-ready code. Reviewer-friendly docs. No excuses left.

If directions are loaded, use them as the quality lens for this polish pass.

## Phase 1: Polish Code

Make the implementation as clean as possible.

1. **Review the diff**
   The diff against main is in your context. Check it against the repo's style guides.

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
   Run the project's test suite and all required lint checks.
   - Follow the repo's documented guidance first (`TESTING.md`, `README.md`, and relevant module docs).
   - Use the repo's standard command entrypoints and CI definitions.
   - Run everything CI enforces for the files you touched.
   Fix failures—determine whether it's broken test or broken code. Add tests for key behavior changes. Keep them focused. Delete flaky tests rather than patching them.

5. **Cleanup**
   - Remove dead code, debug prints, resolved TODOs
   - Remove backwards-compatibility shims that aren't needed (old parameter names, deprecated re-exports, migration code for formats nothing uses)
   - Consistent formatting in changed files
   - No leftover comments like `// TODO: remove this`

## Phase 2: Polish Docs

Make the change easy to review.

1. **Write the design review doc** → `scratch/<branch>-review.md`

   This document helps reviewers quickly grasp the diff:

   | Section | Content |
   |---------|---------|
   | **What was implemented** | Concrete description. "Added X that does Y." |
   | **Key choices** | Decisions made, why, alternatives rejected |
   | **How it fits together** | Architecture in 2-3 sentences or a diagram |
   | **Risks and bottlenecks** | What could break. What's slow. What's fragile. |
   | **What's not included** | Intentional omissions. Scope boundaries. |

   This isn't a changelog. It's a guide for someone reading the PR cold.

2. **Run validation and capture results**
   - Run the "done when" check from the design doc (`scratch/<branch>.md`)
   - If the work has measurable outcomes (performance, accuracy, latency, size, counts), run before/after comparisons and record the numbers
   - If the work is a UI or UX change, capture the key states and interactions
   - Not every PR has metrics — but when they exist, capture them now. The reviewer shouldn't have to reproduce your setup to see the impact.

3. **Write PR copy for ops handoff**

   The PR body is written for an engineer picking this up cold. They're asking:
   - What is the intention of this change?
   - What assumptions does it make?
   - What does it accomplish?
   - How can I tinker with it and evaluate it myself?

   Structure:
   - **Try it!** — lead with this. Concrete commands to run, what the reviewer will see. Make it easy to tinker. If there are metrics, show them here: "Before: X, After: Y."
   - **Intent** — one paragraph. Why this change exists and what it accomplishes. Not a file-by-file changelog.
   - **Assumptions** — what this relies on being true. Environmental, architectural, or domain assumptions the reviewer should validate.
   - **Key decisions** — choices that weren't obvious. What you picked and why.
   - **Not included** — intentional omissions, if any.

   Write to:
   - `scratch/pr-title.txt` — one-line PR title
   - `scratch/pr-body.md` — markdown PR body
   - `scratch/.pr-copy-ref` — current `HEAD` SHA (`git rev-parse HEAD`)

   `lf op land` consumes these files.

4. **Update README and docs**
   - If user-facing behavior changed, docs must reflect it
   - Examples must work. Commands must be current.
   - Check: `README.md`, module READMEs, docstrings on public APIs

5. **Inline documentation**
   - Add comments where the "why" isn't obvious
   - Don't document the obvious. `# increment counter` above `counter += 1` is noise.

6. **Wave alignment** (if running in a wave context)
   - Does the shipped code advance the wave's Goals?
   - Were any known Risks from the wave README introduced or ignored?
   - Are there observable Metrics to note in the review doc?

## Scope

**Polish this branch.** Only code changed by this branch.

**Skip unrelated improvements.** "While I'm here" fixes belong in a separate branch.

**Skip style preferences.** Working code you'd write differently isn't broken.

**Don't gold-plate beyond design intent.** Polish to the design, not past it.

## Output

Phase 1 produces clean, tested code. Phase 2 produces:

- `scratch/<branch>-review.md`
- `scratch/pr-title.txt`
- `scratch/pr-body.md`
- `scratch/.pr-copy-ref`
- updated docs

If nothing needs fixing and tests pass, say so—but still write the design review doc.

## Reference

```bash
git diff main...HEAD     # see what changed
```

Find lint/test commands from repo guidance (`TESTING.md`, `README.md`, docs) and mirror CI checks. A gate that passes locally but fails CI is a broken gate.

## Adaptation

Did you discover a quality check this repo always needs? A linter, a type check, a build step that should run every time? Encode it so the next gate is faster. Most discoveries belong in repo docs (CLAUDE.md, TESTING.md) where all steps can see them. Copy this step to `.lf/steps/gate.md` when the repo needs gate to work differently — a changed workflow, different quality bar, or team preferences about what gate checks and how.
