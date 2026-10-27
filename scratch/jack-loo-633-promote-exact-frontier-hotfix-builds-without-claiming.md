# LOO-633: promote exact-frontier hotfix builds without claiming release authority

Status: implemented and verified 2026-10-26 (draft PR pending). Source: Task definition; evidence in
`scratch/database-locked-5-whys.md` (Why 3).

## Decision

Authority matters only where the store changes. `decide()` no longer rejects a
validation-only build outright:

- Exact frontier, zero drafts → `Promote` for any build.
- Drafts → refused (unchanged), before anything changes.
- `AheadPending` or `Behind` with a validation-only build → refused: only a published
  release changes or passes the frontier; a hotfix needs the exact one.
- `promote` drops its separate "only a published candidate" refusal; the verdict decides.
- Recovery of an exact switch no longer requires published authority.

Provenance: `build.rs` emits `LOOPFLOW_BUILD_LABEL` (`[dirty ]hotfix|development <rev9>
on <git describe tag>`, empty for releases) and `LOOPFLOW_VERSION_LINE`.
`lf --version` prints `lf 0.14.16+e9b2c2eac (hotfix e9b2c2eac on v0.14.16)`; releases
still print `lf X.Y.Z`. `CandidateIdentity.build_label` and `ArtifactSet.build_label`
(Option; older receipts parse as None, older lf ignores it) carry it into the preview,
promote output, receipt and `lf doctor`.

## Delete — do not maintain

- `decide()`'s unconditional validation-only rejection.
- `promote_published_from_installation`'s published-only refusal.
- Authority check in `exact_published_switch_candidate`; ValidationOnly→Development
  mapping in `active_install_matches_candidate`.

Kept: `LOOPFLOW_BUILD_PROVENANCE`/`LOOPFLOW_MIGRATION_AUTHORITY` (release CI needs them).

## Remaining

Check: `cargo test` lib (compatibility, build_info, version, installation) + `global_commands` + `one_machine_tests`, fmt, clippy — pass.
`scripts/test_task_installation.py --repository-upgrade` keeps the env-var route: it migrates.


- Demo: worktree at v0.14.16, two-line fix, `cargo build --release`,
  `lf self install promote --preview`, promote; then the same with a draft (refused).
- Coordinate with #1656 (newcomer install) on `commands/install.rs` / `installation.rs`.
