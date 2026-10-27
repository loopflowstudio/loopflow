# LOO-633: promote exact-frontier hotfix builds without claiming release authority

Status: implemented and verified on the branch 2026-10-26; rebased on main and
hotfix skill updated 2026-10-27. Source: Task definition; evidence in
`scratch/database-locked-5-whys.md` (Why 3). Jack Heart asked (Task comment) to ship
without stopping for review.

## Problem

Promote refused validation-only builds, so the LOO-604 hotfix was rebuilt with
`LOOPFLOW_BUILD_PROVENANCE=release LOOPFLOW_MIGRATION_AUTHORITY=published` (second 5-min
build) and recorded as a release. The same env vars would let a draft-bearing build
advance the store. A hotfix migrates nothing, yet had no supported path.

## Demo

Temporary worktree at this branch's tip (stands in for the tag), two-line fix, plain
`cargo build --release -p loopflow --bin lf`, `./target/release/lf self install promote
--preview` → `VERDICT: promote (no migration to apply)` and `hotfix <rev> on <tag>`;
promote; `lf --version` and `lf self doctor` show the label. Add an empty migration draft,
rebuild, preview → `REFUSED` before anything changes. No v0.14.x tag carries this rule
and the live store is on the v0.14.17 frontier, so a literal "hotfix on a release tag"
is first possible on the release that ships this.

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
`lf --version` prints `lf 0.14.17+e9b2c2eac (hotfix e9b2c2eac on v0.14.17)`; releases
still print `lf X.Y.Z`. `CandidateIdentity.build_label` and `ArtifactSet.build_label`
(Option; older receipts parse as None, older lf ignores it) carry it into the preview,
promote output, receipt and `lf self doctor`.

## Delete — do not maintain

- `decide()`'s unconditional validation-only rejection.
- The published-only refusal in promote (now `promote_from_installation`).
- Authority check in `exact_published_switch_candidate`; ValidationOnly→Development
  mapping in `active_install_matches_candidate`.
- `.lf/skills/hotfix.md` (landed on main in #1712, LOO-635) step 4's env-var build and
  its "wrong provenance label" caveat: a plain `cargo build --release` now promotes.
  Keep step 1's empty migration diff (it catches drafts before a build; promote is the
  enforcement).

Kept: `LOOPFLOW_BUILD_PROVENANCE`/`LOOPFLOW_MIGRATION_AUTHORITY` (release CI needs them;
`scripts/test_task_installation.py --repository-upgrade` migrates, so it keeps them).

## Remaining

- Demo: promote the plain build and show `lf --version` / `lf self doctor`; add an empty
  draft, rebuild, preview → `REFUSED`. Preview of the branch tip already reads `VERDICT:
  promote (no migration to apply)` with `dirty hotfix c683600a5 on v0.14.17` (2026-10-27).
- Coordinate with open #1656 (newcomer install) on `commands/install.rs` /
  `installation.rs`: whichever lands second rebases; no shared API changes.

## Done when

- A draft-free, unreleased build on the store's exact frontier previews `promote (no
  migration to apply)` and promotes without either env var.
- The receipt, `lf --version`, promote output and `lf self doctor` show `hotfix <rev>
  on <tag>`; a release build still shows `lf X.Y.Z` with no label.
- The same build with a draft, or against a `Behind`/`AheadPending` store, is refused
  before anything changes.
- `.lf/skills/hotfix.md` no longer sets the env vars.

Gate check: `cargo test -p loopflow --lib` (compatibility, build_info, version,
installation) + `--test global_commands` + `--test one_machine_tests`, `cargo fmt
--check`, `cargo clippy --all-targets -- -D warnings`.
Check (2026-10-27, merged main 4a85fb3a5a; compress folded doctor's label and renamed the promote fn): focused lib tests + global_commands + fmt + `cargo build -p loopflow` — pass; clippy and one_machine_tests left to gate.
