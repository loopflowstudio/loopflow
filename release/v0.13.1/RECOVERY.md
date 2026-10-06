# Why v0.13.1 was skipped

Investigated October 5, 2026 after Jack Heart requested autonomous prevention.
Jack accepted retaining an incorrect preparation commit in history, followed by
a corrected commit that publishes the same version.

## Evidence and causes

1. **The version PR merged, but the release did not publish.**
   [PR #1426](https://github.com/loopflowstudio/loopflow/pull/1426) merged on
   October 4 at 14:50:56 UTC as `49f8385f0f41194434ce208a2b63ea10b570e723`.
   GitHub's published release list read October 5 goes from v0.13.0 to v0.13.2.
2. **The merged source still needed preparation.** That exact commit contains
   `rust/loopflow/src/store/migrations/drafts/shared_provider_homes.sql`, as well
   as the already-prepared `0.13.1.001_release.sql`. The prepared PR head
   `a495533d3fe7a5c6647e4e84427ee82ec5472043` has no SQL drafts and is based on
   `52ab4a4a5`. [PR #1416](https://github.com/loopflowstudio/loopflow/pull/1416)
   merged the missing draft at 09:19:56 UTC, before the release PR opened at
   14:33:29 UTC. Preparation used an older snapshot; integration changed its
   migration set. Refusing to publish the resulting draft-bearing binary was correct.
   The installed v0.13.0 controller prepared from local `main`; its worktree helper
   discarded `sync_main` errors. This allowed an old base to survive a failed sync,
   although the exact sync error for this attempt is unavailable. PR #1419 already
   replaced that path with a fetched, pinned `origin/main` source; it does not fix
   changes introduced during later integration.
3. **Recovery consumed another version.** In `ops/release.rs`, the merged-source
   inspection failure explicitly called `bump_version(..., "patch")`. The
   deterministic release branch would otherwise find the same merged PR again.
4. **Migration tooling made another cut at the same version impossible.**
   `canonicalize_migrations.py` refused an existing version's second batch, and
   `check_migrations.py` required ordinal `001`. A preparation commit was being
   treated as a completed publication for version numbering.
5. **The prevention contract encoded the wrong outcome.** Recovery tests asserted
   publication of the successor version; `release/README.md` documented that
   behavior. The fix therefore belongs in the controller, migration preparation,
   tests and operating instructions together.

The original controller's retained Exec has no terminal receipt. Its exit cause
is unknown; the source evidence above establishes why the subsequent supported
recovery skipped the number, not why the first process stopped or the exact
failed/ineffective synchronization that left its base stale. Those remain evidence gaps.

## Prevention

An untagged candidate needing correction retains its version. The controller
selects a retry branch derived from the rejected commit, preserving the original
PR and commit. Scheduled recovery retains the rejected selection and publication
inspection while replacing its source at the same version. Unknown or partial
publication does not authorize replacement.

Each corrected release cut appends a migration batch at the next ordinal in the
same version. Earlier canonical SQL and IDs remain unchanged. The existing
migration identity and registry support these ordinals; no store migration is
needed for this repair.

An unfinished tagged release stays pending. The controller reports its recovery
blocker instead of publishing a higher version or rewriting its tag. Candidate
build and publisher preparation remain prerequisites to creating a release tag.
A merged preparation PR does not consume a version number.

Regression coverage exercises same-version manual and scheduled recovery,
partial-publication refusal, tagged-candidate preservation, failed-build
recovery, and late migration preparation without changing earlier SQL.

## Recovery status

Availability recovered through v0.13.2 on October 5 at 07:31:48 UTC, followed by
v0.13.3 at 08:40:22 UTC. Those releases do not fill the historical v0.13.1 gap.
This prevention does not retroactively publish v0.13.1 or relabel newer artifacts.
The source change must land and ship before the installed release controller
uses the corrected behavior.
