# Testing audit redesign

## Intent

Spend local agent time on the smallest proof that can change the next decision.
Let CI and release own the full matrix. Treat real product behavior, deployed
health, and logs as proof rather than an afterthought.

## Changes

The remaining rebase cost deserves its own measured task: join conflict paths,
selected test targets, compilation time, and the following gate/CI result. The
question is whether 3–12 narrow commands buy distinct evidence or merely
recompile the same dependency graph. Preserve the lower-case product family
spelling: `manabot/managym/etude`.

1. Add `scripts/test_time.py` to the developer toolkit as a privacy-preserving
   reader over captured command intervals. Filter by days, repository, or exact
   worktree. Print aggregate categories and skills; never print command text,
   prompts, or output. This is intentionally not a product CLI surface.
2. Add exact-tree reuse to the changed-aware runner. A passing affected-suite
   result may be reused only when the complete committed + dirty + untracked
   tree fingerprint and selected phase set match. Full and host gates always
   execute.
3. Remove the 30-day full-gate budget verdict and its duplicated documentation.
   Keep per-phase timeouts, elapsed summaries, and failure artifacts.
4. Give lifecycle skills distinct scopes:
   - `implement`: focused behavioral proof and Done When only.
   - `compress`: focused proof only for behavior changed by the reduction.
   - `lint`: format/lint only; never tests.
   - `rebase`: focused proof for resolved conflicts; no conflicts means defer
     branch-wide verification to gate/CI.
   - `gate`: reuse exact-tree evidence, run affected suites once, and leave the
     full matrix to CI unless reproducing CI or preparing a release.
5. Add a builtin `testing-audit` skill that repeats the evidence → proof map →
   deletion/redesign workflow.
6. Update `TESTING.md`, `AGENTS.md`, demo guidance, and user docs. Strengthen
   nightly package smoke beyond `lf --version` with safe real CLI reads.
7. Delete the clearest prose-presence test assertions; retain safety contracts
   and executable behavior tests.

## Done when

- `uv run python scripts/test_time.py --days 7 --worktree /path` reports only
  aggregates and handles normalized captures, legacy Codex/Claude captures,
  missing artifacts, and parallel command intervals without leaking content.
- A changed-aware pass can be reused on identical tracked and untracked file
  content. Content or runner-plan changes invalidate it; staging or committing
  unchanged content does not.
- `scripts/test.py --all` and `--ui-host` never reuse results.
- `scripts/test.py --history` and the 30-day budget verdict are gone; phase
  budgets still kill hangs and print elapsed time.
- Builtin testing guidance names focused → affected-suite → CI/release →
  product proof ownership consistently.
- Nightly packages exercise `lf --help` and builtin discovery from the packaged
  executable.
- The testing-audit skill is auto-registered and passes builtin validation.
- Focused Python and Rust tests for the changed infrastructure pass; formatting
  and clippy are clean.
