# Finish global install cutover and verified rollback (ENG-25)

## Problem

W2-319 (#1077, merged `88b295ff`) landed a safe low-level `lf install promote`
transaction — reservation fence, candidate-first activation, immutable
content-addressed staging, migration-after-activation — but nothing *called* it.
`scripts/install.py` still did the machine-global mutations itself:

- `refresh` → `_install_cli_binaries` copied `lf` straight into `~/.local/bin`.
- `local --use` → `_promote` symlinked `lf` and `shutil.copytree`'d the app into
  `/Applications`.

Both bypass the live-body and migration-frontier checks — the exact door the
2026-07-17 incident walked through (a `0.11.026` binary made global against a
`0.11.027` store, breaking every live body mid-turn).

Second gap: `preserve_prior_binary` labels the replaced binary a "prior
compatible" rollback candidate without ever checking it can operate the current
store. The incident *began* with an incompatible active binary, so retained
bytes alone are not a safe rollback target.

## The demo

```
uv run python scripts/install.py local --use
# → builds, then hands the CLI + app + skills to `lf install promote`.
#   Refuses (touching nothing) if a body is live or the store frontier is
#   incompatible; on success prints either
#     "rollback available: lf install rollback --candidate ~/.lf/bin/lf-<digest>"
#   or "prior executable retained as historical bytes (not rollback-compatible)".
```

## Approach

This ports the W2-319 generation-3 repair (preserved as an uncommitted five-file
diff in the terminal-failed W2-319 worktree) into ENG-25, reconciles it against
merged main `88b295ff`, and completes the missing proof.

**One Rust boundary owns every global mutation.** `lf install promote` grows
optional `--app-source`/`--app-target`/`--legacy-app-target`/`--sync-skills`.
`activate_install_then_advance` stages the app bundle (the one fallible copy)
*before* the safety-critical CLI commit, commits CLI + advances the frontier via
the existing `activate_cli_then_advance`, then commits the app by atomic rename
(old app → unique sidecar → new app → drop sidecar + legacy). Skill sync runs
after the lock drops. `install.py` now only *stages branch-local artifacts* and
shells the freshly built candidate: `_promote_with_candidate` for both `refresh`
and `local --use`. `_install_cli_binaries`, `_sync_skills`, and `_promote` are
deleted — no direct Python copy or symlink swap touches a global path.

**Rollback is validated, not assumed.** `lf install rollback --cli-target
--candidate` and the post-promote advertisement both run the *retained binary's
own* `install preflight --json` and repoint only on an exact `Promote` verdict.
`Reject`/`PromoteAndMigrate`/unreadable → refuse (rollback never advances a
migration), and the binary is kept as historical bytes only. `retained_binary_path`
also confirms the candidate is a content-addressed member of `~/.lf/bin`.

## Key decisions

- **`--use` now pins a specific promoted build.** The old symlink pointed
  `~/.local/bin/lf → local-bin/lf`, so any rebuild silently became global — the
  incident's mechanism. Content-addressing pins the promoted bytes; a rebuild
  needs an explicit re-promote. This is the safety improvement W2-319 exists for.
- **App staged first, committed last.** If the app rename fails after the CLI is
  already new + frontier advanced, the global *command* is still the compatible
  candidate (bodies run the CLI, not the app's bundled helper); the stale app is
  a safe degraded state and the staged copy is cleaned up. Preserves the
  post-failure compatible-command guarantee.
- **Call-time global resolution in `install.py`.** `_promote_with_candidate`
  resolves `APPLICATIONS_DIR` at call time and `local()` its bundle spec from the
  module global, instead of freezing them as def-time defaults. Production is
  unchanged; the module globals become authoritative (a latent footgun removed,
  and what makes the entry-point tests able to redirect `/Applications`).

## Scope

- In scope: routing `refresh` + `local --use` through `lf install promote`; the
  app-bundle swap and `lf install rollback` in Rust; retained-binary validation;
  behavioral + unit proof; deleting the bypassing Python.
- Out of scope: rollback of the *app* bundle (the CLI is the store-critical
  surface); the reservation fence and preflight/decide logic (shipped in #1077,
  preserved verbatim).

## Done when

- `refresh` and `local --use` perform no global copy/symlink themselves —
  proven by `test_refresh_routes_default_no_pull_and_custom_dir_through_rust_promotion`
  and `test_local_use_routes_cli_app_bundled_helper_and_skills_through_rust_promotion`
  (the `.rust-promotion-boundary` marker + content-addressed symlink target only
  appear if `install promote` actually ran; a direct copy fails them).
- Incompatible rollback is rejected without repointing the CLI —
  `an_incompatible_retained_binary_is_never_activated_for_rollback`,
  `validate_rollback_verdict_accepts_only_an_exact_promote`,
  `retained_binary_path_rejects_out_of_store_and_mismatched_content_address`.
- App swap is atomic and preserves the compatible command on frontier failure —
  `copy_tree_preserves_symlinks_and_permissions`,
  `commit_app_bundle_replaces_the_old_app_and_removes_the_legacy_bundle`,
  `a_frontier_failure_leaves_the_cli_new_and_the_app_untouched`.
- `uv run pytest python/tests/test_install_script.py` green (10 passed locally);
  full CI green (rust-lint/rust-test are the real verifier — no local cargo on
  this host). Exact-head Project review, then land.

## Notes for review

- Ported via `git apply` of the W2-319 gen-3 diff (the three non-CLI files were
  byte-identical between landing head `6a934332` and merged `88b295ff`; the
  `bin/lf.rs`/`mod.rs` deltas were the unrelated `lf ssh` broker regions and did
  not conflict). Completion work is mine: the two Python def-time-binding fixes +
  the app-routing test repair, and all six Rust tests below the `promote_tests`
  module (the port shipped Python entry-point tests but no Rust proof for the new
  app/rollback code).
