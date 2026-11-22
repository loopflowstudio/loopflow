# Verify LOO-1014 (2026-11-21)

PR #2011 merged as 4cee2ed182; `git diff --quiet 4cee2ed182 HEAD` → tree-equal.

- DESIGN.md on main; VISUAL_DESIGN.md and swift/DESIGN.md gone (`git ls-tree origin/main`); no references to either outside scratch (`git grep`).
- design/prototypes/ present on main.
- GhosttyManager.swift on main: `font-feature = -calt`, `font-feature = -liga`, `adjust-cell-height = 10%`.
- `cd website && uv run pytest -q` → 73 passed.
- PR checks: macos/swift-test pass, macos/loopflow-ui-test pass; merge_group CI success.
- Push CI for 4cee2ed182 ("CI", "macOS on main"): in progress at verify time, not read as passed.
- Not checked here: how the terminal looks with the new defaults (a person, in the app).

Outcome: Verified.
