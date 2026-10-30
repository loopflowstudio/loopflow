# LOO-698: GitHub shows the CI notes instead of the README

Jack, 2026-10-30: go forward with the simple fix only; no README audit.

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
