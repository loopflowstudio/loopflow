# LOO-698: GitHub shows the CI notes instead of the README

Jack, 2026-10-30: go forward with the simple fix only; no README audit.
The Task description still asks for the full audit of root `README.md`;
whether that becomes its own Task is unresolved. That is ordinary later Task
creation, not a choice this PR waits on; the PR title and body describe the
rename, not an audit.

Concept review, 2026-10-30: no change. A `.github/` README reads as the repo's
front page on GitHub; naming the maintainer CI notes `CI.md` makes the root
README the only front door, which is what the growth Wave wants.

GitHub's repository page renders `.github/README.md` in preference to the root
`README.md`, so visitors see "# CI / Scratch, cancellation and macOS path filters".

## Change
- `git mv .github/README.md .github/CI.md`
- Update references: `docs/architecture/delivery.md`, `scripts/check_architecture.py`,
  `scripts/testing.md`, AGENTS.md's module README list (`.github/`). The infrastructure
  Wave memory mention can wait for a Wave pass.
- Leave root `README.md` untouched.

## Demo
GitHub's repo page for the branch shows the root README; `scripts/check_architecture.py` passes.

## Status
Implemented 2026-10-30. Remaining: `.lf/wave/infrastructure/MEMORY.md:893` still links
`.github/README.md`; repoint it in the next infrastructure Wave pass.
Check: `uv run python scripts/check_architecture.py` — pass.
