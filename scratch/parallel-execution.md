# Main implementation handoff

LOO-298 · 2026-09-28. Main owns executable edits, builds, Git and this handoff.
Supervisor owns the other compact scratch documents. Read the full
[contract](data-model-one-table-per.md), [remaining scope](remaining-work.md),
[import obligations](import-preservation.md) and [evidence](evidence.md).

## Current change and proof

Flow selects an exact AgentSession native start under its version/claim and
conversation driver/provider generation. Cursor advancement consumes that turn's
successful completion transactionally in `flow_events`. A surviving engine is
read back after driver loss; the original Exec outcome remains unknown.
Explicit retry after confirmed engine exit releases the selected boundary without
inventing an outcome. `flow resume --retry` retains that instruction through
managed Task dispatch; `task run --retry` and an explicit repair reason use the
same preparation with existing Task adoption/agent/unblock policy.

Automatic retries in one Exec may replace a failed/interrupted selected turn,
retaining every outcome and usage receipt. Reselection clears the failed turn's
decision/router candidate atomically. Live/successful selections, old versions,
claims and provider generations cannot be replaced. Review caught that merely
changing `selected_start` inherited a failed turn's Advance; that public RED is
retained by the supervisor. Missing replacement decision now blocks; a failed
Iterate can be replaced by successful Advance.

Candidate SHA `894aded0d8c465a971d4006ed6016e2c3ae38bbd1d6eb175100c7cadd473d701`:
`.lf/tmp/execution-model/native-recovery-{automatic,both,decision-missing,decision-replace}-combined/results.json`
all pass through actual CLI/Codex0.157.1 with synthetic Responses/private Homes.
Running/completed driver-loss repeats also pass on these identical bytes.
Prior separate candidate passes and original failures remain in evidence.md.

`.lf/tmp/cut-i/flow-navigation-tests.log`: 4/4 pass (exact selection and both hosted
CI failures plus managed retry). Managed proof uses public Flow-to-Task dispatch,
retained adoption refusal and synthetic claimed-successor history consumed by the
shared driver. It does not prove configured managed Codex launch/account continuity.
Earlier compile failures in `flow-final-recovery-tests.log` and
`flow-retry-parity-check.log` remain retained; corrected checks pass.
`flow-recovery-clippy.log`: all-target Clippy passes. Formatting, Ruff, migration
history (55 shipped unchanged) and architecture inventory pass. `canonical-recovery.log`: 5/5 pass on 23 drafts materialized into a disposable
0.12.25 canonical source copy, including managed recovery and empty-draft upgrade.
No broad gate or new hosted CI is claimed.

Against published `7e2101b41`, production +434/-78 (net +356): Rust/Swift +413/-75,
SQL +17/-0, Python scripts +4/-3. Method strips trailing tests, excludes tests/docs,
disables rename accounting and includes the new SQL draft. Receipt:
`.lf/tmp/execution-model/flow-recovery-counts.json`. This is one owner conversion
checkpoint; Runs and their fallback still exist. No deletion or code-complete claim.

## Next dependency

Finish the active final checks, checkpoint this recovery change with Jack's
scratch consolidation, inspect `lf rebase --plan`, integrate main via `lf rebase`,
and publish PR1296. Jack authorized these checkpoints. Main advanced to
`d9632d833c8216b00cde117656d12172eda00c2c` (PR1317): preserve saved Session
executable/Home/database and failed-launch diagnostics during integration and
subsequent Run deletion. No landing, auto-merge, promotion or Flow navigation.

Then finish execution owners: replace current Run publication and decision
membership with AgentSession/FlowSession ownership; mechanical boundaries retain
start/outcome on Flow history; agent outcomes/usage stay on Session history.
Remove Run and its readers only after all import evidence and callers converge.
The remaining-work matrix is still mandatory, including indexed reads, DTO/
Desktop retention, Chapters, dense measurements and configured acceptance.

## Control and proof discipline

Use captured installed control in [parallel-work](parallel-work.md). Never run
source drafts against installed Home. Saved feature invocation remains
`1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`, implement iteration9; no restart selected.
Single build slot; disposable proof Homes, inherited LF/LOOPFLOW authority scrubbed,
four Cargo workers/nice +10. `.lf/tmp/cut-i/run.py` owns proof environment/logs.
Canonical tests materialize only a disposable source copy. Native fixtures copy
the candidate before executing. Do not restore recursive archived scratch logs.
