# LOO-722 discovery repair

## Accepted outcome — 2026-11-04

Jack Heart accepted keeping laptop disk usage low across coding repositories and
tooling, including recurrence prevention and successive usage measurements. A
cleaner PR or one successful invocation does not satisfy that outcome. Repeatable
cleanup belongs to the same opt-in hourly runner, excluded from product packages
and automatic installation. Personal inventories remain private. Accounting keeps
retention preference separate from content type; neither classification grants
deletion authority, and physical reconciliation gaps remain unknown.

Jack Heart's accepted policies and reservations remain unchanged:
4h/1h/15m idle-output windows; verified owner teardown only after 24h; in-use
dependencies preserved; successful evidence 24h, failed/unknown evidence seven
days, saved evidence preserved. Source, unpublished work, durable history,
databases and project-owned stores remain protected. Native cache caps apply even with plentiful
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

## Concept review — 2026-11-04

Outcome: no design change and no open product choice. Jack Heart's accepted
experience remains one explicitly installed hourly cleaner with private receipts
and bounded growth history. The standalone usage guide already describes that
experience and the repaired unknown-age behavior; no new commands, configuration,
skill instructions or persistent lifecycle are needed.

The interaction is install → scheduled inspection → eligible reclamation → read
the receipt and successive measurements. `Candidate` carries a path, category and
owning checkout; discovery does not grant deletion authority. `_reason` applies
retention, reservations, activity and actual Git rules; `_stop_owners` and `_clean`
own verified teardown and removal. `_prune_docker` leaves native image ownership
with Docker. `_record_usage` and the run receipt retain observations separately
from cleanup decisions. Carrying checkout ownership through discovery removes
repeated lookup without adding a user-visible concept or persistent state.

Normal path: an idle, ignored generated root can be reclaimed while a nested
repository uses its own rules. Failure: incomplete discovery remains an explicit
failed run; a missing image timestamp means retain that image, not infer age or
abort unrelated eligible maintenance. Recovery: the next hourly run rediscovers
current state under the same lock and protections. Source, durable evidence,
reservations and unknown-age images survive; the latest receipt and bounded
history may change. A stale receipt cannot establish recovery.

Counterexamples remain acceptance constraints: an independent repository named
`target` must not become disposable through its name; discovery exclusions must
not authorize deleting unignored source; an image without `Created` must not
prevent later eligible images from being considered. The current repair and its
focused tests address these cases. No additional implementation direction follows
from this review. The remaining mismatch is between a successful invocation and
the accepted sustained laptop outcome, already retained below as unverified.
Next proof remains final-byte calendar acceptance and successive measurements;
this review neither repeats execution nor treats delivery as containment.

## Remaining work

The discovery and missing-Docker-timestamp repairs exist in this checkout at
`d5e0b1151b`; publication and final installation remain. Publication review synced
cleanly with main and checked the complete repair diff, including local notes.
Calendar acceptance requires a complete ordinary hourly :17 invocation of those
final bytes, a fresh successful receipt, matching installed digests and unchanged
protected content. The successful manual invocation below used the pre-compress
repair. A read-only comparison during reconciliation confirmed that the scheduled
runner still differs from this checkout; its other four installed files match.

Sustained containment remains unverified independently of discovery performance.
Successive ordinary-run observations must still establish coverage, growth and
target shortfall across coding repositories and tooling, with unknown measurements
visible. Any remaining recurring growth needs attribution and repository-owned
prevention where applicable. Reclaimed allocation and free-space change alone do
not establish this. No new cleanup-policy decision is needed for the current repair.

## Implemented repair

The repair removes repeated ancestor searches in `_builds`, repeated nested-checkout
probes, Git descent into known generated roots and `.lf` state, and Docker's
three-field template that fails on missing `Created`.

Discovery carries checkout ownership in the queue, resolving nested ownership once
when enqueueing each directory. Registered worktrees retain independent coverage.
Git enumeration skips known generated contents, while `_reason` still requires
actual ignore rules and no tracked files before deletion. Original bounds,
symlink protection and incomplete-coverage failures remain. Completed discovery logs
candidate, checkout and directory counts plus elapsed time.

Docker reads JSON metadata, retains unknown-age images and continues maintenance.
Retention cases share one receipt-writing branch with distinct reasons. Tests cover
source roots inside a checkout, independent nested repositories (including a repo
named `target`), real ignore-rule enforcement, and eligible Docker reclamation after
an unknown-age image. Review found no change to deletion authority.

Checks: `uv run pytest python/tests/test_disk_cleanup.py` with inherited `LF_*` unset and closed stdin — 101 passed in 71.90s; architecture check, Ruff check/format for both changed Python files, and `git diff --check` passed. The changed-aware planner selects the entire Python package plus an unrelated CLI build; this bounded repair used the complete cleanup suite locally, leaving broader Python coverage to CI. Final-byte scheduled verification remains with its Graph step.

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
