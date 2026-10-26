# LOO-601: every hidden `__` command earns its place

Accepted direction is Jack Heart's, in the Task brief. Done when `lf help` lists no
hidden subcommand other than `__context-block`, the root has no hidden flag, and
`telemetry-daily` runs through a skill. Status: implemented; gate remains.

## Decisions (2026-10-26)

- `--__cwd` → visible root `--cwd <PATH>` ("Run in this directory; Work still
  comes from `--task` or `--wave`"), conflicting with `--wt` only. Operator default; Jack
  may revise at demo. Without `--task` it enters the directory up front (relative
  paths allowed); with `--task` the Task supplies Work and the flag the directory.
  The relaunch in `ops/human_session.rs` keeps passing both, because a bound
  conversation's Work and cwd can differ.
- `--skill-input` / `--output-schema` are visible root flags with help text.
  Graph steps pass them; a person can too.
- `__transport-serve` → `lf session serve [--terminal] -- <argv>`: "Run the
  process a Session lives in while terminals come and go". It skips the
  repository reconnect, as before. `process_input`'s delegation SQL matches
  `["lf","session","serve",…]`.
- `__pi-event` / `__tool-lifeline` fold into the single provider-hook entry
  point: `lf __context-block --moment pi-event` (lifecycle JSON on stdin) and
  `--moment tool-group --lifeline <FIFO> --provider-pid <PID>`. Neither records a
  Process row any more or runs the repository reconnect (both were per event or
  per bash command). The CLI dispatches both moments directly; `emit_block` stays
  the hook-JSON path only.
- `__telemetry-scorecard` → repo skill `.lf/skills/telemetry-scorecard.md`,
  piping `lf history usage --json --days 0` into `scripts/lifecycle_scorecard.py
  --history /dev/stdin`; `telemetry-daily` is `doctor`, `telemetry-scorecard`, `monitor
  usage --weekly`. The script's `--database` defaults to `$LF_HOME/loopflow.db`;
  `--envelope` is gone (its `metric_observations` was always empty).

## `__repair` → ordinary run (Jack Heart chose option 1, 2026-10-26)

CI repair is an ordinary `lf -b skill ci-fix "<incident>"` run. Admission reserves
the incident for the launcher, starts the run as a detached Session and hands the
reservation to the run's transport owner Process. The message carries the failing
checks with URLs and the JSON answer contract; `RepairCi` now receives the checks.
The next landing check settles an exited run: `process_input(owner)` →
`input_final_answer`, then `finish_repair` writes `repair_finished_at`, the Session,
and the published/blocked conclusion, or why there is none. A run that ended without
a conclusion blocks the landing once; the next check retries with a fresh run,
within `automation.retries`. A published summary stays in the run's Session.

## Other hidden commands and flags (Jack Heart widened scope, 2026-10-26)

Jack: clear wins only; leave the rest for LOO-610 without ceremony.

- Made visible: `lf sync-skills` (people are told to run it) and
  `lf ci watch --parent-pid` (Desktop passes it; a person can too).
- Left for LOO-610: `lf install recover-switch|preflight|advance-switch|promote|rollback`
  (the candidate-binary install protocol), `lf task location --request` and
  `lf machine use --here` (peer relay nonce/fan-out), `lf cron run`/`--scheduled`
  (launchd's entry; its `--wave/--graph` shape predates `cron add <name>`).

`__context-block` stays (Jack's decision).

## Delete — do not maintain

- Done: `Commands::TransportServe`, `PiEvent`, `ToolLifeline`, `TelemetryScorecard`;
  `ops/graph.rs` (scorecard runner, envelope, metric-observation writer and its unit
  tests); the script's `--envelope`/`format_report`; the hidden-scorecard parse test
  (replaced by a test that only `__context-block` is hidden and the root hides no flag).
- Done with `__repair`: `TaskCommand::Repair`, `run_repair`, `process_ci_fix`,
  `incident_landing`, `CaptureHandle::begin_reserved_with_context`, the reservation's
  Session pre-creation and live-provider check; `agent_process_identity` is test-only.
  The store's `handoff_repair`/`finish_repair` stay: they are the reservation handoff
  and the settlement write the ordinary-run path uses (`ops/pr_landing.rs`).
- Done (concept review 2026-10-26): repair admission passes `--output-schema` with the
  conclusion schema (`status`: `published`|`blocked`, `summary`: string) to
  `lf -b skill ci-fix`; the message carries only incident facts and the JSON answer
  sentence (kept for non-Claude providers); the procedure lives in the ci-fix skill,
  whose step 5 names the `--output-schema` conclusion it answers with.

## Remaining

- Concept review (second pass, 2026-10-26): implementation matches the accepted
  decisions; remaining hidden items are exactly the LOO-610 list. No change, no open choice.

- LOO-610 material, no action here: `__context-block` now also carries Pi lifecycle
  events and bash tool groups, so its name says less than it does.
- Gate. Real-Pi `tool-group` admission (the bash guard in `context_pi.mjs`) has no
  headless fixture; it is covered only by the real-Pi probes.

Tests that used `__telemetry-scorecard` as a mechanical vehicle are migrated: a failing
step is `cmd: graph show no-such-graph` (its Task status reads "command exited with
status 2", not the error text), and a long-running owned step is `cmd: session serve --
python3 effect.py`.

Realign (2026-10-26): `tests/fixtures/dto/task_lifecycle.json` still recorded the
deleted `__telemetry-scorecard` step (its contract comparison ignores labels); regenerated
with `LOOPFLOW_UPDATE_LIFECYCLE_FIXTURE=1`. Swift `DTOFixtureTests` decode is left to CI.

Compress (2026-10-26): `settle_repairs` maps the answer straight to conclusion/error; lib pr_landing, integration repair, clippy: pass.

Check (realign 2026-10-26, ci-fix message): lib pr_landing (21), integration `repair` (6), clippy `--all-targets`: pass.

Check (realign 2026-10-26): lib lifecycle fixture test (compare mode) + integration `dto_fixtures`: pass.

Check (2026-10-26): lib pr_landing + parse, integration land repair/waited tests,
`release_tests` repair, `session_lifecycle_tests`, documented commands, CLI reference
(regenerated), clippy `--all-targets`: pass. Earlier pass: lib graph/task-graph-launch/
lifecycle, `process_ownership_tests`, doctor, transport, Pi launch/lifecycle, pytest
architecture/scorecard. Pi lib tests need an isolated `LF_HOME`.

## After completion (Jack Heart, 2026-10-26)

When LOO-601 completes, launch LOO-610 (top-down lf CLI API re-evaluation),
stacked on LOO-519 if LOO-519 has not landed. Its review is decided in its Task
conversation before broad implementation.

Jack Heart (2026-10-26) authorized autoship: when pursue reaches `demo`, take the
`ship` edge without a demo review.
