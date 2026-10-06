# Scheduled public installer verification — October 5, 2026

LOO-285 follow-up to merged PR #1419. The accepted accounting design survives at
`e2eccdd257f0f3e2cf34db7583f7309eb0084394:scratch/account-for-scheduled-release-opportunities.md`.
Jack Heart's October 4 review approval is retained in scheduled-release-demo.md;
it establishes neither installed acceptance nor Task completion.

## Implemented behavior

The former public smoke called the pinned installer with temporary HOME/LF_HOME,
but installation uses getpwuid's account Home and could mutate the release host.
It also hashed the installed entry gate as if it were the native executable.
Installer execution now uses a disposable Ubuntu 24.04 ARM64 container with
copied public artifacts, no host mounts and no forwarded credentials. Smoke
compares manifest-selected CLI bytes against the exact public Linux package and
exercises the installed gate's version, help and list commands. Native macOS
package version/help smoke remains separate.

Candidate preflight is also isolated, resolving the preceding iteration's gap.
Preparation, cached-candidate reuse and publication all check the exact packaged
ARM64 Linux CLI in a new container with networking disabled. Cached receipts do
not bypass preflight. The shared container lifecycle propagates service and
cleanup failures. Host preflight with an LF_HOME override, host installation,
entry-gate hashing and the redundant validation wrapper are removed.

The simulated-service checks cover preparation, reuse, rejected authority,
failed/malformed preflight, unavailable Docker and cleanup failure without host
preflight. The selected-artifact test independently checks manifest digest,
actual bytes and launcher version. Review found the existing manifest's
`selection` owns the artifact set; launcher bytes alone cannot establish it.
No scheduler, settlement, receipt schema or installation routing change is needed.

## Remaining acceptance

Real candidate preflight and public installer execution in Docker remain for
capable gate/CI; the recorded local daemon was unavailable. Linux container proof
does not establish macOS app installation. Native signing/notarization, required
headless Desktop checks, exact public hashes and product identity remain required.
LOO-357 retired UI-host as a prerequisite; #1441 removed the scheduled validator's
obsolete requirement. Public smoke cannot substitute for prepublication checks.

The affected gate and supported installed accounting still need acceptance,
including current telemetry and public artifact/installer proof. Completion
requires two adjacent original configured dues, two distinct automatic executions,
at least one publication, all required verification and no manual repair.
Collapsed misses provide accounting coverage only. All 36 dated telemetry
failures, including the original 35, remain counterevidence.

Release child memory records v0.13.3 shipping #1441 and installed jobs repaired
at unchanged 09:00/10:00 schedules. Its October 5 history read found three
unresolved opportunities with unknown timezone provenance, zero executions and
no qualifying pair. These are dated observations, not fresh readback. Installed
ownership remains Infrastructure; the child is unregistered. Neither schedule
repair nor manual publication read-back supplies an unattended settlement.

No new product decision is needed for the local implementation. Production
release, installation, triggers, schedule changes and Task completion remain
outside this reconciliation's authorization.

Check: recorded `uv run pytest python/tests/test_release_publisher.py -q` — 42 passed, with Ruff passing; prose-only realignment reuses those results; `git diff --check` passed; real Docker execution remains with gate/CI.
