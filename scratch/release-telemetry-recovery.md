# Telemetry prerequisites and automatic recovery

Starting head: `0240d43e7167239b78debf3f4ec0b518a57e4119`.

This slice retains the prerequisite evidence for frozen release coverage and
adds one automatic retry when current telemetry is missing or failed. The full
Task remains incomplete and unpublished.

## Existing owners extended

`ReleaseAttempt.telemetry` holds the observed installed schedule/activation,
Home-local timezone, each original covered opportunity's prerequisite interval
and receipt references, current receipt references, and an optional recovery
receipt. Dates before known installation/timezone observation remain explicit
unknowns. This does not reconstruct replaced or removed telemetry obligations
from today's installation.

The existing atomic obligation writer reserves that snapshot, including recovery
identity, before a child can start. Coverage must match the attempt. Re-entry
cannot replace its retry reservation; an older attempt cannot attach evidence
to its successor. A new release attempt retains all earlier observations and
gets its own retry allowance.

Recovery calls the same installed cron executor as ordinary firings. It retains
placement checks and physical failure receipts. `recovery` is a distinct source;
no scheduled timestamp is backdated and no historical receipt is rewritten.
The existing job lock now also protects telemetry execution and is inherited by
its target. An occupied job defers recovery instead of starting another verifier.
A failed prerequisite retry fails release before selection, with the receipt/log
cause, repair-disposition instruction and next configured due time.

Release history keeps referenced prerequisite receipts outside its display
window. This preserves old failure timestamps and repair dispositions beside a
new successful result. Product settlement remains the existing atomic writer;
this snapshot does not introduce another success authority.

## Review findings

- Receipt timestamps have second precision. Lexical UUID ordering cannot prove
  a same-second pass followed a failure. Selection now prefers Running, then
  Failed, then Succeeded for equal timestamps. A fixture puts a pass at the
  largest UUID beside a failure and still requires recovery.
- A held telemetry job lock is a known continuation. Recovery returns typed
  Deferred at that boundary; other execution errors remain failures. The
  release adds its exact next due time without classifying error text.
- Reservation failure writes a failed physical receipt before returning. It
  cannot leave an unlaunched child represented only as Running.
- Searches found no Swift or Python mirror of these cron/history DTOs. The
  Rust history fixture includes the new snapshot and a separate recovery receipt;
  old schema-1 physical receipt decoding remains intact.

## Focused proof

The joined fixture uses the actual cron executor, built CLI, mechanical release
Flow, Git repositories, Home store and OS locks. It covers publication, no-change,
persistent telemetry failure, successful telemetry recovery, missing telemetry,
historical prerequisite linkage, public smoke failure and missing public stage.

It checks one retry per wake despite multiple collapsed dues, an unchanged old
failure beside successful recovery, a failed retry blocking selection, and an
older linked failure retained outside the history window. Caller HEAD, branch,
raw index and edited/untracked files stay unchanged. Historical schedule and
receipt rows are explicitly seeded in disposable storage; verification,
GitHub and publisher services are simulated.

Separate focused tests exercise immutable retry reservation and rejection of
late writers, plus actual job-lock exclusion with a shell target that writes an
observable file only after ownership becomes available. The existing history
fixture tests retain failure ownership/lateness and pair exclusions.

Final command results are recorded below after completion.

## Remaining work and evidence limits

- Retain/reconcile previous telemetry obligation segments across replacement,
  removal, timezone and Home changes. This slice reports unavailable historical
  authority as unknown; it does not erase that denominator or supply old proof.
- Record dated owning repair dispositions through the supported command. No
  production disposition or accepted Intelligence handoff was performed here.
  All 36 observed failures, including the original 35, remain counterevidence;
  the missing `agent_turns` scorecard query remains the observed blocker.
- The installed target executor is synchronous. One retry bounds cardinality,
  not wall-clock runtime. Bounded target waiting and controller-death recovery
  still need their concrete continuation and interruption proof.
- The real telemetry Flow starts with `doctor`. Recovery does not count as a
  natural scheduled receipt in that existing continuity check. Missing scheduled
  evidence can therefore still fail the actual prerequisite. The simulated
  successful verifier is no proof that the configured Flow will recover, and
  this slice introduces no continuity exemption.
- Closed-obligation continuation, remaining interruption proof, supported
  installation/sync, UI-host/public exact-tag verification and two adjacent
  automatic executions with at least one publication remain mandatory.

No full gate, hosted matrix, installation, cron trigger, production publication,
PM handoff, landing or Task completion was performed.

## Validation results

- `cargo test -p loopflow --test scheduled_release_tests -- --nocapture`:
  passed all eight scenarios, including same-second ambiguity, in 267.10s.
  Compilation took 1m41s; concurrent builds were active. The earlier eight-case
  run before adding the ambiguity case passed in 66.67s.
- `cargo test -p loopflow --lib telemetry_ -- --nocapture`: nine passed, 9.75s,
  including reservation fencing and actual job-lock deferral/release.
- `cargo test -p loopflow --lib ops::cron::history::tests -- --nocapture`:
  four passed, including the updated DTO round-trip and retained late ownership.
- `cargo fmt --check`, `git diff --check`, and
  `cargo clippy --all-targets -- -D warnings`: passed. Final Clippy took 3m29s
  including build-lock waiting.

The joined command started before the final typed overlap adjustment; the
focused job-lock case and final Clippy compiled that adjustment. No full affected
suite was added. These are local implementation receipts, not fresh production
telemetry, configured recovery, UI or publication evidence.
