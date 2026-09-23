# LOO-292 enforced learnings — 2026-09-23

PR-note fallback: the installed CLI exposes no `memory` command (`lf memory
--help` renders top-level help; the command enum has no Memory entry).
`lf pr status` reports no open PR for this serial branch. Retain these notes
for its eventual publication; do not open an evidence-only PR or edit Wave
memory to bypass the missing surface. Existing demo edits remain unclaimed.

- **Preservation must survive the next command.** The incident analysis
  reproduced `lf rebase` retaining an unpublished main commit, followed by
  `lf wt create` discarding it. The merged `ops::checkout::refresh_main` now
  serves ordinary rebase/install/worktree refresh. The regression
  `test_main_preserves_unpublished_commits_and_index` in
  `python/tests/test_checkout_refresh.py` advances upstream between rebase and
  sibling creation, checks both commit ancestries in both checkouts, and checks
  staged, working, and untracked bytes. Future refresh changes must preserve
  this composed outcome; origin equality is not the invariant.
- **Version equality is insufficient installation evidence.** Development
  binaries can report the release version; current control binaries can coexist
  with a missing or stale Mac app. `scripts/install.py` checks the existing
  published preflight identity, both binary versions, and Mac app metadata and
  executable completeness before skipping downloads. The installer regressions
  `test_published_refresh_skips_assets_only_for_current_release` and
  `test_published_refresh_repairs_the_mac_app` enforce the fast-path boundary.
  Reuse existing installation authority rather than inventing a success cache.

The demo records 43 passing checkout/installer fixture cases and real candidate
checkout/package use. These are retained observations, not tests rerun by this
documentation pass. Merged source is still distinct from installed capability:
the recorded published v0.12.19 predates #1273 and rejects `install schedule`.
Installed scheduling, actual scheduled/wake catch-up, and explicit recovery
remain unproven in `demo-installed-laptop-refresh.md`. No release, installation,
activation, material implementation change, or Task completion was attempted.
