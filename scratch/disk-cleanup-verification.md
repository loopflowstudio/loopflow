# LOO-722 discovery repair

## Remaining work

Continue the authored Graph: gate the affected suites, publish this Task's next
PR, reinstall final bytes through the existing opt-in installer, and verify a
complete ordinary hourly :17 invocation with matching digests and protected-content
checks. The installed runner predates the compress changes below. The successful
manual launchd invocation does not establish calendar acceptance of final bytes.
Sustained disk containment remains unverified independently of the discovery fix.

Jack Heart's accepted coding cleanup policies and reservations remain unchanged:
4h/1h/15m idle-output windows; verified owner teardown only after 24h; in-use
dependencies preserved; successful evidence 24h, failed/unknown evidence seven
days, saved evidence preserved. Keep source, unpublished work, durable history,
databases and project-owned stores. Native cache caps apply even with plentiful
free space. Non-coding content remains observation-only except the separately
approved stale Chrome signing copies; never stop Chrome. Reclaimed allocation,
free-space change and measured growth remain distinct observations.

PR #1820 is merged delivery history, approved by Jack Heart after demo. Its merge
is `6f0d98e0168d1ab04456fd9d627fce93b4fa3f26`. This checkout integrated main via
`lf sync --manual`; no second Task or competing Graph was created. Earlier accepted
design, policy and demo details survive at `7e969d4f6e:scratch/loo-722.md`,
`7e969d4f6e:scratch/cleanup-policy-decisions.md`, and
`7e969d4f6e:scratch/disk-cleanup-demo.md`. Earlier claims that no PR exists are stale.
Existing Manabot/Kata prevention and separately owned runtime work are not duplicated.

## Repair and deletion scope

**Delete — do not maintain:** repeated ancestor searches in `_builds`, repeated
nested-checkout probes during traversal, Git descent into known generated roots
and `.lf` state, and Docker's three-field template that fails on missing `Created`.
These paths have been replaced; no remaining deletion target or exclusive fixture.

Discovery carries checkout ownership in the queue, resolving nested ownership once
when enqueueing each directory. Registered worktrees retain independent coverage.
Git enumeration skips known generated contents, while `_reason` still requires
actual ignore rules and no tracked files before deletion. Keep the original bounds,
symlink protection and incomplete-coverage failures. Completed discovery logs
candidate, checkout and directory counts plus elapsed time.

Docker reads JSON metadata, retains unknown-age images and continues maintenance.
Retention cases share one receipt-writing branch with distinct reasons. Tests cover
source roots inside a checkout, independent nested repositories (including a repo
named `target`), real ignore-rule enforcement, and eligible Docker reclamation after
an unknown-age image. Review found no change to deletion authority.

Checks: `uv run pytest python/tests/test_disk_cleanup.py -k 'discovery or ignored_data or docker or xcode or swiftpm or embedded_checkout'` with inherited `LF_*` unset and closed stdin — 17 passed; Ruff format/check and `git diff --check` passed. Broader acceptance belongs to gate; installation and final-byte calendar verification belong to the later verification step.

## Evidence and limits — 2026-11-04

The first ordinary post-merge run began at 15:17 PST and reported discovery's
120-second timeout at 15:21:19; it ultimately exited 1 with discovery and overall
budget failures. Its previous receipt/exit zero did not describe that invocation.
The 16:17 calendar run completed discovery and removed 8,911,110,144 allocated
bytes, but exited 1 because a real Docker image omitted `Created`. Its free-space
increase was 7,671,988,224 bytes, measured separately.

Profiling found Git's ignored-file enumeration dominated discovery, plus 17,324
repeated checkout-ancestor searches. Foreground probes took 5–7 seconds and
background-policy probes 17–29 seconds; neither reproduced 120 seconds. The precise
load-dependent timeout cause remains unproven. A paired background-policy comparison
before compress found identical 173 candidates across 90 checkouts and 17,325
directories: 17.24 seconds before, 13.26 after. These single observations do not
establish sustained performance.

The pre-compress repair was installed through the opt-in installer. A manual
`launchctl kickstart` ran 16:30:14–16:33:10 PST, exited zero with no failures, and
completed discovery in 12.4 seconds (173 candidates, 90 checkouts). Nine acceptance
checks confirmed a fresh successful receipt, finished launchd process, unchanged
hourly schedule/configuration, six protected specimen files unchanged, two reserved
roots and the durable database retaining filesystem identities, all five installed
files matching that checkout version, and unknown-age Docker retention. No eligible
allocation was removed; free space rose 105,918,464 bytes independently.

Historical merged verification passed 172 tests before the timeout. The earlier
final-byte calendar acceptance passed fourteen assertions, including verified
TERM/KILL cleanup and neighboring-process/source/history/reservation preservation;
it removed 180,224 allocated bytes while free space fell 1,544,781,824 bytes. The
14:17 receipt later removed nothing while free space fell 2,016,219,136 bytes, with
zero target shortfall. Its history held 13 samples (388,257 bytes); the newest
measured 47 roots, left 82 unknown, and had 46 last-known comparisons (8 growing,
2 shrinking). Overlap and differing measurement times prevent summing these roots.
These are historical observations, not proof of containment or of this final repair.

Private raw evidence remains under the existing maintenance evidence root's
`20261104-LOO-722/merged-verification/` and `discovery-repair/`. Before shortening
this note, its complete uncommitted contents were preserved as
`discovery-repair/verification-before-compress.md`; that directory also holds
profiles, earlier receipts, usage and protected-content snapshots. No personal
inventory is committed. No probe or cleanup process was started by compress.
