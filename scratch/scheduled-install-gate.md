# Scheduled public installer verification — October 5, 2026

LOO-285 follow-up to merged PR #1419. The accepted accounting design survives at
`e2eccdd257f0f3e2cf34db7583f7309eb0084394:scratch/account-for-scheduled-release-opportunities.md`.
Jack Heart's October 4 review approval is retained in scheduled-release-demo.md.

## Defect and implementation

The former public smoke called the pinned installer with temporary HOME/LF_HOME,
but installation uses getpwuid's account Home and could mutate the release host.
It also hashed the installed entry gate against the native executable.
The implementation moves installer execution into a disposable Ubuntu 24.04
container with copied public artifacts, no host mounts or forwarded credentials.
It compares the selected
CLI artifact and actual bytes against the exact public Linux package; exercise
the installed gate's version, help and list commands. It keeps native macOS package
version/help smoke separately. Docker failure remains a failed verification.
No scheduler, settlement, receipt schema or installation routing change is needed.

Delete — do not maintain: host installer invocation with fake HOME; entry-gate
hash comparison; singleton binary loops and their obsolete host-install fixture;
candidate preflight on the host with an LF_HOME override. These paths are removed.
Preserve pinned public downloads, complete asset hashes, native
smoke, exact product identity and failed-publication evidence.

Counterexamples: installer selects wrong bytes; correct gate selects wrong version;
installer fails after publication repair; container service unavailable; container
cleanup fails. None may produce a new verified receipt. Container checks prove
Linux CLI installation, not macOS app installation; existing signing/notarization
and required headless Desktop CI remain required. UI-host receipt was retired by
LOO-357 and must not be restored from the historical design.

## Remaining acceptance

Real container execution belongs to gate/CI while Docker is unavailable locally.
Supported installed accounting, current telemetry, public proof and two adjacent
original configured dues with distinct automatic executions (one publication,
no manual repair) remain unproved. All 36 dated telemetry failures survive.
No production release, installation, trigger or schedule change is authorized here.
Candidate preflight now extracts the exact ARM64 Linux archive into a disposable
Ubuntu 24.04 container with networking disabled, no mounts and no forwarded
environment. Preparation, cached reuse and publication share this path. The
container lifecycle also owns public installer smoke; cleanup failure propagates.
Focused simulated-service checks cover preparation, reuse, rejected authority,
failed/malformed preflight, unavailable Docker and cleanup failure without issuing
host preflight. Public smoke cannot substitute for prepublication checks.

Release child memory (October 5) records v0.13.3 shipping #1441 and installed
jobs repaired to use the machine gate at unchanged times. Installed history still
had three unresolved opportunities with unknown timezone provenance, zero executions
and no qualifying pair. Those are dated child observations, not a new status read.
The child Wave remains unregistered; no ownership transfer is established.

Review confirmed the smoke reader follows the machine manifest's `selection`
owner and checks selected bytes independently of the launcher. Cached receipts
cannot bypass fresh preflight. Compression removes the validation wrapper and
publication's duplicate archive scan; callers share the candidate check directly.
Selected-artifact and entry-gate behavior stays in the isolated smoke test.

Check: `uv run pytest python/tests/test_release_publisher.py -q` — 42 passed; Ruff and `git diff --check` passed; real Docker candidate/installer execution deferred to gate/CI (daemon unavailable), not public proof.
