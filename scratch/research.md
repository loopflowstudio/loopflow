# LOO-285 kickoff evidence

## Observations

Captured on 2026-09-23 America/Los_Angeles (2026-09-24 UTC); exact capture time
is in `evidence/baseline-summary.json`. Source base is
`88cf10641b0e88fc4bfcf504ef62bed4aa057539`.

| Read/probe | Retained evidence | Meaning and limit |
| --- | --- | --- |
| `lf cron list --wave infrastructure --json` | `evidence/installed-crons.json` | Two loaded jobs on `home_39860354aaca640c2ccb50bf6ca609d8`; release is still a skill, telemetry a flow. Shows installed truth, not execution success. |
| `lf cron history --wave infrastructure --days 35 --json` | `evidence/cron-history.json` | 70 rows: 36 telemetry failures, 33 release target successes, one release target failure. Earlier chapter observation was 35 and 32/33 respectively. |
| `lf release status` | `evidence/release-status.txt` | v0.12.19; hosted workflow succeeded; GitHub Release exists. Command observes remote state but supplies no opportunity/verification join. |
| Existing publisher receipt | `evidence/publisher-v0.12.19.json` | Commit `bca3f35ac7d4ad24ebed2a30481d0b7ba3a86bde`, workflow `35894647999`, seven artifact hashes, ten completed stages. Copied without running publisher. |
| Latest telemetry log excerpt | `evidence/telemetry-latest.txt` | Continuity passes, scorecard fails with missing `agent_turns`. Log is shared append-only output; association with latest cron receipt is contextual, not an existing structured stage join. |
| Read-only SQLite `EXPLAIN SELECT 1 FROM agent_turns` | `evidence/baseline-summary.json` | Reproduces missing table on the Home DB named in the telemetry log. No table data, secrets, or state were changed. |
| `uv run pytest python/tests/test_release_publisher.py -q` | 6 passed, 1.10s | Existing test proof only: native matrix/archives, candidate binary handling, and exact artifact preparation. No live signing/publication performed. |

The raw evidence includes ordinary local paths and durable ids, not credentials
or environment dumps. No cron trigger, sync, install, release run, PM mutation,
or worktree mutation was performed.

## Source findings

- `rust/loopflow/src/ops/cron.rs`: `run_cron` writes a schema-1 receipt before
  placement checks, calls `spawn_cron_target`, and settles from exit status.
  `spawn_cron_target` scrubs the environment and runs `lf --wave … --batch
  skill|flow …`. No cron id is passed to the child. Activation comes from plist
  metadata or oldest matching receipt. `trigger_cron` uses `kickstart -k`.
- `rust/loopflow/src/lf/commands/doctor.rs`: `check_continuity` checks only the
  latest due interval and accepts an exact scheduled receipt regardless of
  target outcome. `scheduled_on` uses local time, first ambiguous occurrence,
  and no occurrence for a nonexistent local time.
- `rust/loopflow/src/ops/flow.rs`: the existing op executor supports
  `release run` directly. No new scheduler target variant is needed.
- `rust/loopflow/src/ops/release.rs`: `ReleaseRunOutcome` has no-change,
  released, and resumed. `ReleaseReceipt` is a returned value, not a durable
  opportunity settlement. General failures become `OpsError`. Early no-change
  and same-tag resume precede the configured verification hook.
- The release path already reconciles exact candidate refs and generated
  worktrees under `acquire_worktree_lease`. Its publisher checks a non-draft
  GitHub Release after the command returns. There is no whole-selection lock
  across concurrent release invocations.
- `scripts/publish_release.py`: candidate receipt binds commit/workflow/hashes
  and required preparation stages; final receipt is atomically written under
  `.lf/logs/release.<tag>.json`. It is written after publication, leaving a
  crash window. Public read-back/smoke is not represented in that receipt.
- `scripts/lifecycle_scorecard.py:load_usage` queries `agent_turns` and
  `agent_invocations`; the observed missing table is not merely an obsolete
  error string in Wave memory.
- `engine/git.rs:sync_main` resets main and uses stashes; its overlapping-path
  preservation test expects edits to remain in a stash. That is weaker than
  unchanged caller bytes/index and no manual repair.

## External semantics checked

Apple's [timed jobs guide](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html)
distinguishes sleep wake-up from power-off, which waits for a future designated
time. Apple's [launchd manual source](https://github.com/apple-oss-distributions/launchd/blob/main/man/launchd.plist.5)
documents coalescing multiple calendar intervals on wake. These support explicit
due-time accounting; they do not prove current host power history. No actual
sleep, power cycle, or live launchd test was performed.

A web fetch of the public v0.12.19 release page returned a cache miss. It adds
no verification. The installed `lf release status` observation and local
publisher receipt are the positive evidence retained here.

## Review findings resolved in the design

1. A timestamp join cannot distinguish delayed wake, manual trigger, or retries.
   Introduce durable opportunity identity and explicit attempt linkage.
2. Treating publication/no-change as a bare release return skips important
   proof on early returns. Require one settlement routine for all outcomes.
3. A parent-only lock is inadequate if a publisher child survives. Preserve
   child stage ownership and prove that crash boundary.
4. Snapshotting only current schedule loses removed/changed obligations.
   Persist closed obligation segments and historical unknown coverage.
5. “No new code, so green” is false while required scheduled verification is
   red. Keep those failures, ownership age, and acceptance blocking visible.
6. Making retrospective release health change doctor's exit creates a cycle:
   release awaits telemetry, which runs doctor. Keep that history in the release
   view while preserving existing doctor's scheduling checks.
