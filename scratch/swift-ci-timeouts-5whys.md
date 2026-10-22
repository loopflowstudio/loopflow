# 5 whys: swift-test timeouts on main and across PRs (2026-10-21)

## What failed

`swift-test` hit its 20-minute step limit on main (run 39663098280, 19:36Z) and on
#1635, #1639, #1624, #1607 and others into 2026-10-22. Each timed-out job held one
of roughly four macOS runners for 20 minutes, so 20+ macOS jobs queued and landings
stalled for about five hours. The log of each timeout named a different "last
started" test (theme navigation, Git planning), so the failure read as a hang.

**Recovery:** #1645 removed the lf-world Swift tests. Its queue run took 4 minutes
for the step (161 s of tests). Jack Heart cancelled the doomed PR runs through this
session to let #1645 through. LOO-564 brings the tests back.

## Causal chain

1. **The Desktop test step exceeded 20 minutes.** Annotations: "timed out after 20
   minutes". `events.jsonl` (artifact `swift-tests-<run>`) shows the unfinished tests
   were lf-world tests still building their worlds:
   `LifecycleWorldTests.waveInventory…` on main and
   `WorkDensityWorldTests.sidebarDensity` on #1635. Swift Testing runs in parallel,
   so the last line in the log is not the stuck test.
2. **Three PRs in one day each added minutes.** The step on main took 2–5 minutes on
   the morning of 10-21, 11 minutes after #1617 (density world, 17:39Z) and 13–14
   minutes after #1619 and #1621. Each world test built its own world. There were two
   helpers, `BuiltWorld` (#1617) and `World.shared` (#1619), and no sharing between
   them. `lf-world build density` takes 184–278 s on an M-series laptop.
3. **Each PR passed alone, because the only bound was a hang guard.** Twenty minutes
   was 4–10× the step's normal time. A PR could add 10 minutes and still pass. #1617's
   first PR run *did* time out (10-20 22:11Z, 20 min). Its next run was a main merge
   that passed at 11 minutes, and the timeout was treated as noise.
4. **The cost was known and written down, and nothing waited for it.** #1617's PR
   body: "The density world still takes about 7.5 minutes to build, against a 60 s
   target … The swift CI step now pays for that build inside its 20-minute cap. Jack
   Heart has two options: accept about 450 s in CI …, or open a Task." The PR then
   landed through auto-merge. A decision addressed to a person in PR copy does not
   hold landing, and an agent's autonomous landing does not surface it.
5. **A timeout says nothing useful.** The step log names no test, and diagnosis needed
   the event stream. This session first misread it as a hang, as did earlier
   readers.

Amplifier: macOS runner concurrency is about four jobs. Each timed-out job burned 20
runner-minutes, and every queued PR was doomed by the same tests, so recovery meant
cancelling other people's runs.

## Prevention

- **Done in this PR — a pull-request test budget.** Building Desktop is split from
  running its tests. The test step allows 5 minutes on `pull_request` and 20 in the
  merge queue and on main. A PR that slows the suite fails on its own PR. The queue
  is not ejected by runner variance. Proof: this PR's own `swift-test`.
- **Done in this PR — name the culprit.** On failure, `scripts/swift_test_report.py`
  writes the unfinished and slowest tests to the job summary. Checked against both
  failed runs, it names the world tests.
- **Proposed — a decision in PR copy holds landing.** When `pr-message` writes a
  choice addressed to a person ("Jack Heart has two options"), `lf land` should ask
  that person or file the decision before auto-merge, not land past it. This needs
  Jack's call on the mechanism.
- **Proposed — a required-check timeout is a finding, not a flake.** ci-fix and land
  guidance: investigate a timeout on a required check before a later green run lands.
- **Tracked — LOO-564** shares one world build per recipe per run, keeps one helper,
  and replaces `BuiltWorld`'s `waitUntilExit()` after `await`, which TESTING.md warns
  can hang. **LOO-562** covers per-test limits and path filtering.

## Open

- Rust integration tests have no comparable per-PR budget. Shard times were not
  measured here.
- Whether 5 minutes flakes under runner variance. The 161 s baseline gives about 1.9×
  headroom; watch the first week of PRs.
- `lf arm` reported "already armed" for #1642 while GitHub showed no auto-merge
  request. The PR was in the queue (UNMERGEABLE against #1645's deletion), so arm
  was right. The report is confusing when a queued PR shows no auto-merge request.
