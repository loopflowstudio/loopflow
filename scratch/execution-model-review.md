# Execution model: supervisor proof checks

2026-09-28 · LOO-298 · Review during the active implementation, not a final
verdict. Keep the full design's completion matrix; these are concrete checks
against the current draft and must not become a smaller finish line.

- The inspection proof executed successfully: one `session list` invocation
  records one completed Exec and no Run work. It does not establish all command
  coverage, Task Started behavior on an existing Task, interruption or import.
- The actual-engine ownership fixture reserves its Session and changes drivers
  through store methods. Its real provider launches real `lf` child processes,
  so it can prove the provenance edge. It does not prove public `session connect`
  or restart, normal launch recording, or Flow consumption. Keep those public
  lifecycle proofs in the remaining implementation.
  The executed receipt `.lf/tmp/cut-i/exec-driver-handoff.log` reports both
  inspection and actual-engine tests passing in 29.06 seconds. Inspection of
  the assertions confirms original parent → replacement driver → historical
  original parent for a stale provider, plus direct-child classification and
  rejection of the stale driver claim. Provider responses are synthetic.
- Preserve a live provider while there is temporarily no driver. The earlier draft
  `driver_in` returns `None` as soon as `driver_exec_id` is null, discarding the
  stored provider origin/generation from that read. `claim_session_driver` then
  initializes both generations and assigns a new provider origin. When adding
  release/reconnect, this required proof that a driverless but live engine
  retained its provenance. The implementation now keeps nullable driver identity
  inside the retained provider/generation record and exposes a fenced release.
  The expanded actual-engine test includes that released-driver interval;
  `.lf/tmp/cut-i/exec-release-proof.log` reports four cases passing in 36.77
  seconds, including success, command failure and interruption. The public
  connect/restart path still needs its own proof.
- Historical `escalated` events do not by their name alone establish an OS
  interruption. The current writer uses escalation for a Flow parked before
  completion. The migration selects `node='run'`, so inspect the historical
  Run-level contract before mapping every such event to interrupted. The draft
  now preserves unknown completion for that historical classification. New
  interruption comes from the explicit termination hook.
- The current interrupted cleanup stores an outcome but signal is unknown
  because the shared ctrlc callback receives no signal identity. Preserve that
  missingness. The CLI does explicitly choose exit 130; distinguish that known
  exit choice from an inferred signal. Repeated terminal evidence must not
  rewrite the first outcome, and a failed receipt write must remain visible.
- The nested-runtime regression passed in
  `.lf/tmp/cut-i/exec-runtime-proof.log` (one test, 3.12 seconds): inner success
  leaves the outer Exec open; its subsequent failure supplies the single
  terminal result. This is a focused lifecycle result, not a full gate.
- Installation/bootstrap handling must retain its own store authority checks.
  Document the precise recording limits before claiming every lf process is
  logged; do not access an incompatible installed database just to log refusal.

Installed Task control recovered after Jack reported a repair: fresh Session
and Task reads succeed and retain the same implementation Run. The earlier
failure remains recorded in [the control-store observation](control-store-observation.md).
No source binary, promotion or draft removal is authorized against that installed
Home. This review creates no second writer and changes no execution claim or
Flow cursor.

## Empty-inventory startup measurement

Four sequential source-binary `session list --all --json` calls against a
disposable Home returned `[]`, exit zero and empty stderr. First initialization
took 3.799 seconds; subsequent calls took 2.469, 2.476 and 2.370 seconds.
Receipt: `/tmp/loo298-exec-latency-zev1ow10/receipt.json`; copied binary SHA-256
`c460d301f993b54c05a6ab53ac2dfb1aeb89d570c88fad630a046cd0c692ff9a`.
The fixture retained only PATH/LANG/TMPDIR before setting private LF_HOME and
LF_DB_PATH, ran outside a repository, and had 73.83 GiB free. No installed Home
or provider was used. The binary was copied before measurement, so subsequent
worker builds cannot change these measured bytes.

This is a small empty-inventory startup sample under shared host load, not a
dense-inventory benchmark, percentile claim or comparison with the previous
release. Repeated development-schema validation is a lead from the earlier CPU
sample; these timings alone do not establish its contribution. Retain this
baseline when implementing the existing performance requirement.

## Public connect must transfer the ability to send turns

Source observation: `harness/codex.rs::send_request` queues provider RPCs on the
current transport, and `start_inner` owns stdio plus process-group cleanup.
The new driver compare-and-set is not yet consumed by this transport. The native
socket fixture demonstrates that a second client can resume the active thread;
it deliberately closes the first client before continuing. That proof therefore
does not demonstrate exclusion of a still-connected old client after handoff.

The accepted one-driver contract requires a public-path proof with the old
client retained across transfer: it can no longer start/steer a turn or mutate
Session state, while the new driver can continue and the existing provider turn
and sibling conversation survive. Passive display alone must acquire no claim.
Choose the smallest transport/client-ownership change that achieves this; a SQL
claim rejection by itself cannot establish exclusion of native RPC writes.
This is a remaining implementation/proof obligation, not an observed provider
failure or a request for another product object.
