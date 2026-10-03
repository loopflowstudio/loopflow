# Published installation and laptop schedule — 2026-10-02

## Current contract

The latest supplied Task direction supersedes the package/checkout refresh and
hourly-default requirements in the [older design](make-laptop-refresh-and-lf-installed-schedule-proof.md).
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

## Operator actions and remaining proof

The plist and loaded job remain absent. The unbound coordinating conversation
can perform the already-requested activation using the installed CLI:

```sh
/Users/jack/.local/bin/lf install schedule
/Users/jack/.local/bin/lf install schedule
plutil -p ~/Library/LaunchAgents/com.loopflow.refresh.plist
launchctl print gui/501/com.loopflow.refresh
tail -n 80 ~/Library/Logs/Loopflow/refresh.log
```

The second invocation should report already scheduled without reloading.
Verify `ProgramArguments` points to the installed `lf` followed by `install`,
`RunAtLoad=true`, and the weekly calendar is Weekday 1, Hour 9, Minute 0.
Check preserved `LF_INSTALL_DIR`, no source WorkingDirectory or Python, and
both output paths pointing to `~/Library/Logs/Loopflow/refresh.log`.
`weekly` is an equivalent explicit positional argument; do not use `--every`.

Record activation time, launchd run count/last exit, and new log output. A run
caused by bootstrap is RunAtLoad evidence, not proof of a later login or wake.
Observe a natural login, Monday 09:00 local calendar firing, or wake after a
missed calendar opportunity; correlate new runs and successful installer output.
Manual kickstart remains manual evidence. Do not force sleep/logout or damage
artifacts. Installation scheduling no longer updates main or packages.

An additional unbound `lf install` can establish repeat/no-op execution and
explicit recovery from the previously absent automatic opportunity. Preserve
before/after artifact and history baselines; this does not manufacture proof
of an upgrade or repair. Fresh-account public bootstrap, actual upgrade/repair,
populated-history preservation across a change, interactive Mac app acceptance,
and natural scheduled/login/wake execution remain unproven here.

Original checkout proofs remain in the [September 23 note](demo-laptop-refresh.md).
None were rerun or upgraded from fixture evidence. No other checkout, installed
artifact, schedule, or saved Flow was changed in this contribution. LOO-292 stays open.

## Human decisions

No new product decision is needed for weekly schedule activation or read-only
inspection. Jack's supplied correction removes the daemon requirement; it does
not accept the remaining demo or authorize restarting the saved Flow. App
experience acceptance requires actual observation. The separate Flow continuity
repair remains with its existing owner; this demo does not select Stop & restart.
