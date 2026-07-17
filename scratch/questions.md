# Rebase blocked: live sibling agent owns this worktree

## What I was asked to do
Rebase `jack-heart/authorize-global-lf-promotion-against-3` onto `origin/main`.
Conflicts were expected in install.rs, install.py, test_install_script.py —
main's ENG-25 (#1082) reworked the same install/promotion boundary.

## What the rebase resolves to (computed + verified, not applied)
My tip `8d52c0700` already equals `origin/main` + a coherent delta:
- install.rs: app-bundle stage validates its bundled helper IS the promoted
  candidate (`validate_staged_app_helper`); `read_binary_preflight` parses
  candidate+verdict; `render_retained_prior` prints the cli-target; rollback
  routed through `rollback_from_store`. App/rollback tests folded into three
  signature-aware cases.
- build_info.rs: `MigrationAuthority` gains `Deserialize`.
- scripts/install.py, test_install_script.py, test_release_automation.py,
  release/README.md: matching caller/test updates.

Six files differ from origin/main (mod.rs and bin/lf.rs already match ENG-25).
On the ENG-25 base this content BUILDS (`cargo build` clean) and PASSES
(10/10 `promote_tests`, 20/20 Python install tests, fmt clean).

The clean rebase = reset to origin/main + re-apply those six files from
8d52c0700 as one commit. Do NOT hand-resolve the three-way conflict: main's
old-signature AppPromotion tests can't coexist with the new required
`expected_candidate`/`expected_verdict` fields — 8d52c0700 already consolidated
them.

## Why I stopped (blocker)
This worktree (`/Users/jack/src/loopflow.authorize-global-lf-promotion-against`)
is under active concurrent contention:
- PID 30697: `lf __task ts_a56be8a6227d4d5e822b93b57f522f91 --generation 11`
- PID 31187: its agent — "Continue the preserved directive v9 and reduced
  worktree only. Finish..."

That live agent repeatedly reverted my install.rs/build_info.rs edits mid-session
(the wave-memory "concurrent editing corrupts a file" hazard). It is deliberately
REDUCING this PR under directive v9 — consistent with the wave-chat governance
finding that W2-319 PR3 publishing/promoting conflicts with incorporated v6 and
must preserve v6 unless Developer Efficiency explicitly replaces it.

Racing to restore the full promotion delta would (a) corrupt the live agent's
in-flight work and (b) re-introduce exactly the scope the v9 reduction is
removing. One writer per worktree: the running Task owns the .git sequencer.

## Recommendation
Let generation 11 (directive v9) finish its worktree reduction and land/rebase
this branch. If a full-scope rebase is later wanted, the six-file re-apply above
is the deterministic recipe. Did not commit or push.
