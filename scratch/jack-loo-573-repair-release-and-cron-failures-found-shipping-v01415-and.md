# LOO-573: Repair release and cron failures from shipping v0.14.15 and etude v0.2.1

Status: items 1–5 implemented 2026-10-24 (item 4 intervention included), uncommitted; item 6 investigated, not
reproduced. Jack Heart requested one autonomous Task
covering six defects; fix each at its cause and make lf adapt rather than guard.

## Problem

Shipping Loopflow v0.14.15 and manabot's etude v0.2.1 surfaced six failures. The
first matters most: since the Flow→Graph rename installed, every scheduled job on
Jack's Machine (infrastructure `telemetry-daily`, `release-run`) fails daily, and
`lf cron list` / `lf task automation` error out. The store migrated; files outside
it did not.

## Demo

After landing and `lf install` of the release carrying this change, on Jack's
Machine with the untouched 0.14.14-shaped plists, obligations and receipts:

```
lf cron list                      # lists both infrastructure jobs, latest receipts shown
launchctl kickstart gui/$UID/loopflow.cron.infrastructure.telemetry-daily
lf wave cron history -w infrastructure --graph telemetry-daily   # new receipt, graph form
grep -c LoopflowFlow ~/Library/LaunchAgents/loopflow.cron.*.plist  # 0 after convergence
```

and from etude's main checkout on a feature branch, `lf release notes` writes
`RELEASE_NOTES.md` into the checkout the command ran in, built from
`origin/main`, while a malformed obligation from another repository is present.

## Risk findings (investigated 2026-10-24)

- **Released shapes on disk** (Jack's `~/.lf`, read-only inspection):
  plists carry `LoopflowFlow`, `LoopflowTargetKind=flow`, no `LoopflowCommand`
  (the legacy command fallback derives `--batch flow -- <name>`), and
  ProgramArguments `lf wave cron run --scheduled --wave W --flow G`. Both
  obligations carry `"flow"`; all 52 receipts carry `"flow"` and
  `"target_kind": "flow"`. `read_receipts` fails on the first unparseable
  receipt, so receipts break `inspect_cron`/`lf cron list` as much as the plist.
  Newer 0.14.14 plists with `LoopflowCommand` would hold a `"flow"` subcommand
  token; 0.14.15 rejects `lf flow` ("skill or graph not found: flow").
- **launchd runs stale ProgramArguments until the job is reloaded.** Rewriting the
  plist file alone does not change what launchd executes; reloading from inside
  the running job would kill it. The plist invokes the stable entry gate
  `~/.lf-machine/install/gates/1/lf`, which reexecs the selected install, so the
  new binary does receive the old arguments. Therefore the new binary must parse
  `--flow` on `lf wave cron run` until every loaded job has been reloaded.
- **`lf install` does not reliably run the candidate binary.** The candidate runs
  `install advance-switch` only when the store is not exact; a release without a
  migration never executes new code during install. Convergence cannot hang on
  an install hook.
- **Second failure layer before the rename binary:** on Oct 23 09:00/10:00 the
  0.14.14 runner failed `flow not found: <name>` because main had already moved
  `.lf/flows/` → `.lf/graphs/`. Transient skew, closed by 0.14.15; recorded only.
- **launchd reports `last exit reason = OS_REASON_CODESIGNING` for both jobs**
  (runs=2 since boot Oct 22 12:48; no log output or receipt after Oct 23 10:00).
  A throwaway launchd job executing the same gate binary with `--version` exited 0
  at 2026-10-24 16:23, and `codesign -v` passes (adhoc, linker-signed). The kill
  is not reproducible now; most likely tied to the Oct 23 19:06 install replacing
  the gate. Not fixed here; the demo's `kickstart` step is the check. If it
  recurs after install, file a separate Task with the `launchctl print` evidence.
- **Merge-queue dequeue is distinguishable.** PR #1654's timeline:
  `AddedToMergeQueueEvent` 20:12:44, `RemovedFromMergeQueueEvent reason=manual`
  20:13:13 (actor jacklionheart), `AddedToMergeQueueEvent` 20:13:19 (the run's
  re-arm, same account), merged 20:23:15. Earlier, `AutoMergeDisabledEvent reason
  "Manually disabled by user"`. Actor cannot distinguish the run from Jack (same
  account); `reason` can.
- **Packaged-app hang:** reproduced by Jack 2026-10-23 — unsigned renders, signed
  `com.loopflow.mac` hangs while Desktop runs, signed with another bundle id
  renders. The mechanism inside AppKit/LaunchServices is not identified; the bundle
  id is the demonstrated variable.

## Approach

### 1. Cron state converges on the Graph form (highest priority)

Readers accept the 0.14.14 shapes; writers emit only the current shape; every
lf path that reads a legacy record rewrites it.

- `ObligationSegment.graph` and `CronReceipt.graph`: `#[serde(alias = "flow")]`
  (precedent: `home_id` → `machine_id`). `CronTargetKind`: serde alias and
  `FromStr` accept `flow` → `Graph`.
- `read_cron_spec`: `LoopflowGraph` falls back to `LoopflowFlow`. A plist that
  has `LoopflowFlow` is a 0.14.14 plist: its `LoopflowCommand`, or the derived
  command, has the subcommand token `flow` replaced with `graph`, and the result
  must parse with `parse_cron_command`.
- `lf wave cron run` accepts `--flow` as a hidden alias of `--graph`, so loaded
  jobs run until reloaded.
- Convergence: one `cron::upgrade_installed(launch_agents_dir)` under the
  obligations lock rewrites each legacy plist (`render_plist` from the normalized
  spec, same `LoopflowActivatedAt`), each legacy obligation (`save`) and the job's own
  legacy receipts (`write_receipt`). Identity is unchanged: labels, receipt
  directories, trigger and lock fingerprints use the same strings.
  - `run_cron_recorded` calls it before reading the spec: rewrite files, do not
    reload launchd (the job is running). The next load uses `--graph`.
  - `list_crons` / `reinstall_crons` (`lf cron list`, `lf cron sync`,
    `lf task automation`, `lf doctor`) call it and reload rewritten jobs through
    the existing `add_cron` unload/load.
- Removal: aliases and the `--flow` flag are the shared-file exception in
  AGENTS.md (files the released lf and the branch build both read). Mark each with
  the removal Task, filed in this change: remove once installs at or after this
  release have reloaded every job (one release later, after a cron sync in
  `lf install`'s settle path or a reboot).

Test: an inline 0.14.14 fixture in `ops/cron.rs` tests (beside the neighboring
released-shape test) holding the released plist (no `LoopflowCommand`), one with
a `flow` `LoopflowCommand`, an obligation and a receipt, in the 0.14.14 writer's
shape (`git show v0.14.14:rust/loopflow/src/ops/cron.rs`). Fails before the change
with `missing required LoopflowGraph metadata`. Receipts are rewritten only when
finished; a Running receipt keeps its writer, and `lf cron list` does not reload
a rewritten job whose latest receipt is running. After: `list_crons` returns both jobs
with their latest receipts; files on disk contain no `LoopflowFlow`/`"flow"`;
`activated_at`, labels and opportunity ids are unchanged; `lf wave cron run
--scheduled --wave W --flow G` parses (CLI parse test).

### 2. One unreadable obligation stops only its own repository

`accounting::read(home)` becomes `read(home, repo)`. Obligations from different
repositories never link (replacement requires equal repo), so a per-repository
read is exact, not a filter. Each file is parsed first as JSON to learn its
`repo`; a record for another repository is skipped without being fully parsed.
A record that names this repository, or whose `repo` cannot be read, still fails
with its path — silently skipping a release obligation would lose accounting.
`read` canonicalizes the repository it is given (a path that no longer resolves
matches as recorded), so callers pass `spec.working_directory`, `receipt.repo`,
the release's main repo or `history.rs`'s repository directly.

Test: home with a malformed obligation for repo A and a valid one for repo B;
`record_intervention(home, B, ...)` succeeds and records; the same call for A
fails naming the file. Fails today (any malformed file aborts).

### 3. `lf release notes` uses release run's base and writes where it runs

`release_notes` fetches `origin/<default>` exactly as `release_run` does
(`fetch_inheriting` with the same refspec), collects changes from the base tag to
`origin/<default>`, and runs `run_release_notes_stage` in `repo`'s worktree root
(`git rev-parse --show-toplevel` of the invoking checkout), not `main_repo`.
Config and tags still resolve through the main repo (shared refs). The
checkout's own `RELEASE_NOTES.md` and `release/unreleased/` are the promoted
inputs — the person chose that checkout.

Test: `TestRepo` with origin, main checkout on a feature branch with an extra
commit, linked worktree as the invoking checkout; stub the notes agent stage as
existing release tests do. Expect files written in the linked worktree, none in
the main checkout, and the collected range excluding the feature commit.

### 4. A dequeue during `lf release run` stops the run

`wait_for_pr_merge` re-arms auto-merge whenever no merge request is observed,
every 10 s. Change: when `request.is_none()`, read the PR's latest
`RemovedFromMergeQueueEvent` / `AutoMergeDisabledEvent` (GraphQL timeline). If it
is newer than the run's own arm and its reason is manual (`manual`, `Manually
disabled by user`), return a terminal error: "release PR #N was removed from the
merge queue by <actor> at <time>; the release stops. Run `lf release run` to
resume." Other reasons keep the current path (failing gate → `NeedsRepair`,
otherwise re-arm). The run records its arm time when it enables auto-merge.
A fresh `lf release run` arms again — that is a new explicit request.

Implemented: `pr::manual_merge_removal_since` reads the last 20 removal events;
`wait_for_pr_merge` checks it before each re-arm and advances `armed_at` after
each Loopflow arm, including repair's `reconcile_armed_pr`. `armed_at` is the
local clock compared with GitHub's timestamps; a dequeue within clock skew of
Loopflow's own arm is not distinguished.

Concept review (2026-10-24): a person's dequeue is a person's intervention on
the release opportunity, the same concept as a manual `lf release
tag`/`publish`/`run`. Implemented: the stop returns `OpsError::ManualMergeRemoval`
(PR, actor, time, queued); `accounting::settle_error` owns the scheduled
error→outcome mapping and, on that variant, records an intervention naming the
actor and PR before settling `Failed`, so `history.rs::qualifying` excludes it.
Tests `a_manual_dequeue_is_an_intervention_in_only_its_scheduled_opportunity`
(accounting) and `a_scheduled_opportunity_stopped_by_a_manual_dequeue_cannot_qualify`
(history: a dequeue-stopped attempt followed by a verified publication in the same
opportunity yields no qualifying pair).

Test: `pr.rs` unit test over the timeline parser with PR #1654's events (manual
dequeue after the arm → stop with actor; GitHub's `merged` removal and removals
before the arm → none; a manually disabled auto-merge → stop). The poll loop's
stop/re-arm branch has no direct test.

### 5. Packaged-app check cannot collide with a running Desktop

`_verify_app_resource_self_containment` launches a copy of the signed app:
copy the bundle into the temporary check directory, set `CFBundleIdentifier` to
`com.loopflow.mac.release-check`, re-sign the copy with the same identity,
entitlements and hardened runtime (no `--timestamp`; the copy never ships), and
launch the copy's executable. The shipped bundle is untouched and its signature
stays valid.

Test: `python/tests/test_release_loopflow.py` with `run`/`run_capture` mocked
(no codesign, no launch): the launched executable's `Info.plist` has the
check bundle id and the original `dist/Loopflow.app/Contents/Info.plist` still
reads `com.loopflow.mac`.

### 6. `lf session list --available` agrees with Desktop

Not fixed; no reproduction. On 2026-10-24 the CLI's `--available --all` set and the
paged `session list --json --page` set Desktop reads both held the same two manabot
conversations; LOO-568's two Sessions were active and not Available in both. Both
paths judge Available with the same SQL (`available_sql`) and liveness reader
(`agent_process_evidence` → `OsProcess::read`). Unverified candidate: a CLI run
inside a sandboxed agent shell cannot read other processes' identity, so every
AgentProcess is Unknown, never Live, and nothing is Available. Next step if it
recurs: run `lf session list --available --all --json` from the same shell and
from a plain terminal, and compare `agent_process_evidence`.

## Alternatives considered

- **Hand-edit Jack's files / ask for `lf cron add`.** Excluded by the brief.
- **Rewrite only at `lf install`.** Install runs new code only for store-changing
  releases, and cannot reload jobs owned by another repository's checkout safely
  before the new binary is active. Rejected as the sole path.
- **Plist ProgramArguments independent of the target name** (e.g. `--label`).
  Would stop future renames from breaking launchd but needs the same migration
  now; deferred — not required for this outcome.
- **Item 4 by "seen queued, now gone" heuristic.** Misclassifies GitHub's own
  failed-check dequeues and misses a dequeue before the first poll; the timeline
  `reason` is authoritative.
- **Item 5 by quitting Desktop first** (install already does this). Interrupts
  Jack's work for a check that needs no shared identity. Rejected.

## Scope

Exclusions: the `OS_REASON_CODESIGNING` launchd kills (evidence above; separate
Task if they recur); redesigning cron identity; etude-specific release config.

Delete — do not maintain: none newly; the aliases and `--flow` flag added here
carry their removal Task.

Forbidden outcomes: a second cron reader for legacy files (normalize in the one
reader); rewriting obligations under a different id or activation time; fatal
reads of other repositories' records; notes written into the main checkout from
a command run elsewhere.

## Done when

- Fixture of released 0.14.14 cron files lists, runs and converges (test above).
- Malformed foreign obligation does not block a release in another repository.
- `lf release notes` from a linked checkout writes there, from `origin/<default>`.
- A manual dequeue during `lf release run` ends the run with the dequeue named.
- The packaged-app check launches under `com.loopflow.mac.release-check`.
- Item 6: a fix with a test once reproduced, otherwise the written reason above
  carried into the PR body.
- Gate: `cargo test -p loopflow --test integration cron` plus the new unit tests,
  `uv run pytest python/tests/test_release_loopflow.py`, clippy/fmt.
- After landing and install: Jack's existing telemetry-daily and release-run run
  from launchd without `lf cron add` (demo `kickstart` + receipt).

## Remaining work

- PR body carries item 6's not-reproduced reason.
- Item 6: reproduce before fixing (see above).
- After landing and install: the demo (`lf cron list`, `kickstart`, receipt).
- LOO-574 (filed): remove the Flow readers and `--flow` once every job is reloaded.

Note: `lf release notes` includes only commits that have reached origin.

Checks (gate 2026-10-24, inherited `LF_*` unset): `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass; `cargo test -p loopflow --lib -- ops::cron ops::pr ops::release lf::navigation` 152 passed; `cargo test -p loopflow --test integration cron` 1 passed; `cargo test -p loopflow --test release_tests` 74 passed after the merge-wait gh stub learned the timeline query (`release_run_` rerun 20 passed); `uv run pytest python/tests/test_release_loopflow.py` 13 passed; ruff check/format clean. RELEASE_LOCK_PLACEHOLDER
