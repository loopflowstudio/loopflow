---
requires: code on branch
produces: polished code, scratch/<branch>-review.md
---
Make the branch as ready to ship as possible, and as easy for reviewers to evaluate as possible.

## Goal

Polish isn't "tests pass." Tests passing is table stakes.

Polish means: the code is as good as it can be given the design intent, and a reviewer can understand the change in one read.

Ship-ready code. Reviewer-friendly docs. No excuses left.

## Phase 1: Polish Code

Make the implementation as clean as possible.

1. **Review the diff**
   The diff against main is in your context. Check it against repo style guides (CLAUDE.md, STYLE.md).

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

4. **Tests**
   Run the project's test suite. Fix failures—determine whether it's broken test or broken code. Add tests for key behavior changes. Keep them focused. Delete flaky tests rather than patching them.

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

2. **Update README and docs**
   - If user-facing behavior changed, docs must reflect it
   - Examples must work. Commands must be current.
   - Check: `README.md`, module READMEs, docstrings on public APIs

3. **Inline documentation**
   - Add comments where the "why" isn't obvious
   - Don't document the obvious. `# increment counter` above `counter += 1` is noise.

## Scope

**Polish this branch.** Only code changed by this branch.

**Skip unrelated improvements.** "While I'm here" fixes belong in a separate branch.

**Skip style preferences.** Working code you'd write differently isn't broken.

**Don't gold-plate beyond design intent.** Polish to the design, not past it.

## Output

Phase 1 produces clean, tested code. Phase 2 produces `scratch/<branch>-review.md` and updated docs.

If nothing needs fixing and tests pass, say so—but still write the design review doc.

## Reference

```bash
git diff main...HEAD     # see what changed
uv run pytest tests/     # run tests (or project's test command)
```
