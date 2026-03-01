# Review: Doc Accuracy Pass + Worktree Improvements

## What was implemented

Two interleaved workstreams:

**Documentation accuracy (03-accuracy sprint).** Systematic fix pass across 8 doc files. All code examples in `docs/` now use correct APIs and match the actual codebase:

- `docs/waves.md`: All 5 stimulus examples fixed from `update_wave(..., stimulus=...)` to `add_stimulus(...)`. Multiple stimuli section restructured with `remove_stimulus()` and proper `update_wave()` for pausing. Flow name description fixed ("build" not "ship").
- `docs/config.md`: Flows described as YAML (was Python). Gemini scope note added.
- `docs/index.md`: Flow file extension `.yaml` (was `.py`). `build.yaml` content corrected (added missing `lint`). `lf build` invocation (was `lf --flow build`).
- `docs/getting-started.md`: `gate` replaces `polish` in the feature workflow (polish is a planning step, not a test runner). Steps table updated.
- `docs/lf.md`: Misleading path comment removed from step resolution example.
- `docs/lfops.md`: Prune dirty definition updated to reflect `is_clean_ignoring_scratch` behavior.

**Worktree pruning and rotation improvements.** Three related fixes:

1. `is_clean_ignoring_scratch()` — new git helper that ignores untracked `scratch/` entries when checking worktree cleanliness. Leftover scratch directories from landed waves no longer block worktree pruning.
2. `squash_merged` field on `WorktreeState` — separates squash-merge detection from fast-forward merge detection. Display code shows "squash-merged" status distinctly. Pruning logic uses both signals.
3. Worktree rotation bases on feature branch — `rotate_worktree` now passes the feature branch to `create_with_schema`, so the next wave iteration starts from where the last one left off, not from main.

**Step prompt improvements.** "Complete over incremental" principle added to kickoff, review, and review-design. Verification step added to update-wave workflow. Action language sharpened throughout.

## Key choices

**Surgical doc fixes, not structural rewrites.** The doc architecture is sound — problems were specific wrong values at specific locations. Each fix is traceable to a design doc item.

**`gate` over `lint` in getting-started.** `gate` is the step users will actually run in a shipping workflow. It runs the full quality check. `polish` was misdescribed as a test runner.

**Gemini scope note over Gemini removal.** Gemini works for `lf` commands but not `lfd` sessions. The note makes this clear without removing accurate references.

**squash_merged as a separate field.** Previously squash-merge detection was folded into `merged`. Separating them gives better display output ("squash-merged" vs "merged") and lets the pruning logic handle each case precisely.

## How it fits together

The doc fixes are independent leaf changes — each file is corrected to match the actual API/CLI. The Rust changes form a small stack: `is_clean_ignoring_scratch` feeds into worktree state, which feeds into pruning and rotation display. The step prompt changes are independent additions applied consistently across `.lf/steps/` and their Rust builtin copies.

## Risks and bottlenecks

- `is_clean_ignoring_scratch` only filters `?? scratch/` lines (untracked). Modified tracked scratch files still count as dirty, which is correct behavior.
- The worktree rotation test creates real git repos in tempdir. It exercises the core logic but not the full `land` workflow (which requires a remote).
- Step copies (`.lf/steps/` vs `rust/.../builtins/steps/`) have pre-existing terminology divergence (sprint vs item in update-wave). This branch maintains existing conventions in each copy.

## What's not included

- Automated doc validation tooling (good future work, noted in design doc).
- README steps table expansion (intentionally out of scope — README stays scannable).
- Gemini session harness implementation (the gap is in the harness, not the docs).
- Wave file deletions (`wave/docs/01-setup.md`, `02-docs.md`, `03-accuracy.md`) reflect completed sprints being cleaned up per update-wave protocol.
