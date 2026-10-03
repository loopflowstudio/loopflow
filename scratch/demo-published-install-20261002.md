# Published installation and laptop schedule — 2026-10-02

## Current contract

The latest supplied Task direction supersedes the package/checkout refresh and
hourly-default requirements in the [current plan](make-laptop-refresh-and-lf-installed-schedule-proof.md).
`lf install` updates published machine artifacts only; `lf rebase` owns preserving
checkout updates. Scheduling is opt-in login plus weekly (Monday 09:00 local),
with positional daily/hourly/5min alternatives. Jack requested this bounded
contribution while the managed Flow's separate control-plane dependency is repaired.

## Live observations

At 2026-10-02 22:07 UTC, this Task checkout was clean at
`6d6be01b1476fa17049ad937ac7b492cfed6b314`.

- Installed `/Users/jack/.local/bin/lf` reports 0.12.29 and exposes
  `lf install schedule [weekly|daily|hourly|5min]`, default weekly.
- `gh release view --repo loopflowstudio/loopflow` reports
  [v0.12.29](https://github.com/loopflowstudio/loopflow/releases/tag/v0.12.29),
  published 2026-09-30T23:45:16Z. The old latest-release dependency is superseded.
- Real installed preflight exits successfully: published candidate revision
  `61d21f88564783a5fc8f63e385ec035f096fc069`, Home database
  `/Users/jack/.lf/loopflow.db`, exact frontier `0.12.29.001_release`,
  executable compatibility `compatible` with 33 references, verdict `promote`.
  [Raw JSON](demo-install-20261002-preflight.json) establishes inspection
  reachability and compatibility, not a completed promotion or preservation audit.
- Historical probe: installed `lfd --version` fails: `install artifact set
  published-c6052b6645b3666944bffc54b419e940dd7a94ba488f032741e3ccfc70c2ae84
  is missing role Daemon`. **The inference that this means an incomplete install
  is superseded:** the published release retires the separate daemon executable.
- `/Applications/Loopflow.app` reports 0.12.29. Its MacOS directory contains
  `Loopflow` and `lf`; presence and metadata do not prove complete app acceptance.
- The refresh plist is absent and `launchctl print gui/501/com.loopflow.refresh`
  reports no service. This is an absent automatic opportunity, not a failed run.

## Earlier Task-bound attempt (not an installation defect)

`lf install` exits 1 before repair, both in this checkout and from `/tmp`:

> Error: Task Work cannot change the machine installation. Run installation outside the Task checkout without a Task --as declaration.

See [checkout attempt](demo-install-20261002-first.log) and
[outside-checkout attempt](demo-install-20261002-outside.log).
The released `ops/task.rs::require_unmanaged_checkout` explicitly refuses a
resolved managed Task. Changing cwd did not remove this contribution's Task
authority. No context was stripped to bypass that boundary. No schedule was
activated. The earlier classification of the installation as incomplete was wrong.

The post-attempt active receipt SHA256 is
`83ec8b930f2dc2999933b3339f2b63787eab01a00862d3ce714d94a5ed9826b5`.
There is no before hash, so this is a baseline, not an unchanged-receipt claim.
No production binaries, receipts, Sessions, or other checkout were deliberately
edited; no release, worker, publication, or Task closure was initiated.

## Correction and read-only verification — 22:12 UTC

Jack supplied operator evidence from the unbound coordinating conversation:
`lf install` succeeded with `Published release v0.12.29 is already installed.`
This is reported operator execution, not an installation rerun by this Task or
a new human acceptance decision. No daemon repair is required.

Read-only source inspection confirms `ArtifactRole::RetiredDaemon` in published
v0.12.29 (`61d21f88564783a5fc8f63e385ec035f096fc069`) and locally fetched
`origin/main` (`101a19b8c5008705a6a527779c6fd6666f0c2637`). Its comment limits
the role to decoding and digest verification of retained pre-cutover sets.
No remote freshness claim is made for that main reference.

The active published receipt lists CLI, app, and app helper `lf`. SHA256 checks
match every corresponding installed executable to that receipt:

| Installed executable | SHA256 |
|---|---|
| Selected CLI | `c6e0b58785253efdd1a149d275ca2534adf1e80d35e05ea8619215c256b89026` |
| `/Applications/Loopflow.app/Contents/MacOS/Loopflow` | `fab7fdd2fd08066d23bcc30e5b8179b256874e39b9ba2dd0a02413e1b45e2e98` |
| `/Applications/Loopflow.app/Contents/MacOS/lf` | `9be6cffa4a5dfdbbb0d52c25a5b9492af20822a8c9942323c884e50b0b46f8a0` |

App metadata reports 0.12.29 and executable `Loopflow`; both app executables are
arm64 Mach-O files. The bundled `lf --version` reports 0.12.29.
`codesign --verify --deep --strict --verbose=2 /Applications/Loopflow.app`
passes, including the bundled helper. This establishes installed artifact
completeness and signature integrity, not interactive app acceptance.
The active receipt hash still matches the earlier baseline above.

Read-only SQLite inspection finds 71 AgentSessions, 48,987 session events,
35 FlowSessions and 58 flow events. These are a point-in-time inventory, not
a before/after migration comparison. LOO-292 retains 20 Task events dating
from timestamp 1790186004, two directly bound AgentSessions, its merged PR #1273,
and unpublished serial PR 2. `lf task status ... --json` reads these successfully.
The saved Flow `migrated-task-task_1b419970ebf8419cad62475186c529c1-1790200620`
remains current at step 0, iteration 0, position version 1, without a pending
review Session. Status still reports the separate blocker: “Flow position
predates executable invocation storage (previous step 4).” No restart,
replacement, completion, or repair of that Flow was attempted. Historical
presence does not prove that every pre-upgrade record was preserved or resumable.

## Gate readback — 2026-10-03 04:08 UTC (October 2 local)

Installed CLI, app metadata and bundled helper report 0.12.31. Read-only
`lf install preflight --json` returns published revision
`a278d6bc1bd4373f78f27027b8ae249100ef14d3`, exact production frontier
`0.12.31.001_release`, 33 compatible executable references and verdict promote.
The active selection is published and selects `/Users/jack/.lf/loopflow.db`.
All three installed executable hashes match that receipt:

| Role | SHA256 |
|---|---|
| CLI | `ae80e6c730bd1adaa19aa5e39e80aab7de0abd0b39a73ea0824859608cf5ef75` |
| App | `11cb980867e14cd39b18a26ed9a90d2a973079d13fd46e5da7fcbc8a572546c6` |
| App helper | `8ecf8ee6aa8350597a1162ab8a929c268c3e01f2ea7105919ef69809e9df398d` |

`codesign --verify --deep --strict --verbose=2 /Applications/Loopflow.app`
passes. This is current artifact integrity, not interactive acceptance or a
before/after migration comparison. Jack's latest steer separately reports the
coordinating Wave's successful published upgrade and main-Home migration.
That October 2 migration memory is not present in this checkout's Wave memory;
its contents are not independently reconstructed here.

The [operator activation](operator-install-proof-20261002.md) supersedes the
22:07 absent-job observation. `plutil -p` now confirms installed lf plus `install`,
RunAtLoad, Weekday 1 / Hour 9 / Minute 0, LF_INSTALL_DIR, both refresh.log paths,
and no WorkingDirectory or Python. `launchctl print gui/501/com.loopflow.refresh`
still reports one run, last exit 0, currently not running. The log still contains
only the v0.12.29 already-installed success. No additional natural login, calendar
firing or wake execution is established. Schedule help accepts the four positional
cadences; its usage renders the newer `lf home install schedule` spelling.

Current `lf task status task_1b419970ebf8419cad62475186c529c1 --json` reads
successfully and identifies gate running. The old restart failure above remains
a historical observation. No Flow restart or review completion was performed
by this gate.

## Remaining operator proof and human judgment

The [current plan](make-laptop-refresh-and-lf-installed-schedule-proof.md) retains
public-channel fresh install/repair, populated-history preservation, installed
checkout catch-up and natural login/scheduled/wake proof. Initial job success
cannot substitute for these outcomes. Jack's app experience acceptance and the
authored demo review remain separate from that operator work. No new product
decision is needed to gather the outstanding evidence. LOO-292 stays open.
