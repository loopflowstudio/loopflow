# Keep the version when release preparation needs correction

Jack Heart requested autonomous investigation, implementation and delivery on October 5, 2026. Incorrect earlier release commits may remain; a later corrected commit must publish the same pending version.

## Observed incident and causal chain

1. GitHub releases read October 5 show v0.13.0 followed by v0.13.2 and v0.13.3, with no v0.13.1. PR1426 merged October 4 at14:50:56Z as49f8385f0f41194434ce208a2b63ea10b570e723, but that commit retains migrations/drafts/shared_provider_homes.sql. It was not a publishable source snapshot.
2. The prepared branch lacked a migration already on main before the PR opened; integration changed the prepared snapshot. inspecting the merged source correctly refused a draft-bearing binary. No hosted v0.13.1 candidate appears in the latest release workflow history.
3. release_run_inner explicitly bumped the version when that merged inspection failed. The original merged PR was reused by deterministic branch name, so the workaround escaped it using another version.
4. canonicalize_migrations also refused a second batch in the same version, even when the version was never published. check_migrations encoded that restriction. Preparation was treated as publication for numbering purposes.
5. Recovery tests asserted a successor version and release/README.md documented it. The automation was following the encoded contract; correcting only a prompt would leave the failure intact.

Sources: https://github.com/loopflowstudio/loopflow/pull/1426 ; exact merge tree above; rust/loopflow/src/ops/release.rs; scripts/canonicalize_migrations.py; scripts/check_migrations.py; release/README.md; wave/infrastructure/release/MEMORY.md. Successful v0.13.2 andv0.13.3 publication repaired availability, not the missing number. The first controller's exit is not established by the retained nonterminal Exec; do not invent its cause.

## Repair

Keep an untagged candidate's version through corrections; select a deterministic retry branch from the rejected commit. Preserve earlier commits and candidates. Scheduled recovery retains its replacement evidence while using the same version. Never advance past an unfinished tagged release; preserve it and report its exact recovery blocker. Published/partial/unknown publication cannot authorize replacement.

Allow sequential canonical batches within one version, preserving every earlier batch byte-for-byte. Existing append-only migration IDs already contain ordinal. Tests prove new drafts can be prepared at the same version and prior SQL remains unchanged. No schema migration or live Home access is needed for this source repair.

Do not retroactively point v0.13.1 at current 0.13.3 artifacts or change the latest release. Historical backfill requires its own exact-version artifact build/publication; this change prevents future gaps.

## Proof

Exercise manual and scheduled recovery, repeat interruption, immutable tagged candidates, partial publication, and same-version migration preparation. Focused Rust/Python tests, formatting and all-target Clippy; delivery CI owns its matrix.

Review: retained exact-tag safety, external-publication uncertainty and append-only migration identity. No new persistence model. Historical tag repair remains explicitly blocked rather than silently renumbered.

Checks: release_tests (69 + new patch regression), scheduled_release_tests (10 + corrected saved-candidate regression), Python migration tests (48) passed; fmt/Ruff passed; final Clippy pending.
