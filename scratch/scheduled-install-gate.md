# Scheduled public installer verification — October 5, 2026

LOO-285 follow-up to merged PR #1419. The accepted accounting design survives at
`e2eccdd257f0f3e2cf34db7583f7309eb0084394:scratch/account-for-scheduled-release-opportunities.md`.
Jack Heart's October 4 review approval is retained in scheduled-release-demo.md.

## Defect and implementation

Public smoke calls the pinned installer with temporary HOME/LF_HOME, but machine
installation uses getpwuid's account Home. It can mutate the release host.
It also hashes the installed entry gate against the native executable.
The implementation moves installer execution into a disposable Ubuntu 24.04 container with copied
public artifacts, no host mounts or forwarded credentials. It compares the selected
CLI artifact and actual bytes against the exact public Linux package; exercise
the installed gate's version, help and list commands. It keeps native macOS package
version/help smoke separately. Docker failure remains a failed verification.
No scheduler, settlement, receipt schema or installation routing change is needed.

Delete — do not maintain: host installer invocation with fake HOME; entry-gate
hash comparison. Preserve pinned public downloads, complete asset hashes, native
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
The candidate-preflight helper also uses LF_HOME while reading OS-account
installation state; it is read-only, but cannot prove fresh-Home acceptance.
That preparation path needs reconciliation independently of public smoke.

Simulated review found the machine selection wraps its artifact set under
`selection`; the smoke reader follows that existing owner. The launcher remains
a launcher, and neither its bytes nor its version alone establish selected content.
No publication or review completion was performed.

Check: `uv run pytest python/tests/test_release_publisher.py -q` — 37 passed;
Ruff and diff checks pass; real Docker installer execution deferred to gate/CI
(Docker daemon socket unavailable), not recorded as a successful public proof.
